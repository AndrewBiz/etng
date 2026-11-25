//! Streaming JSON writer that mirrors `exiftool -j` field order and formatting.

use std::collections::HashSet;
use std::io::{self, Write};

use crate::compat::REPORTED_EXIFTOOL_VERSION;
use crate::formatting::format_float;
use crate::model::{FileReport, SystemFields, Tag, Value};
use crate::options::{GroupMode, ReadOptions};

pub struct Emitter<W: Write> {
    out: W,
    first: bool,
    opts: ReadOptions,
}

impl<W: Write> Emitter<W> {
    pub fn new(out: W, opts: ReadOptions) -> Self {
        Self {
            out,
            first: true,
            opts,
        }
    }

    pub fn begin(&mut self) -> io::Result<()> {
        self.out.write_all(b"[")
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.out.write_all(b"]\n")
    }

    pub fn record(&mut self, report: &FileReport) -> io::Result<()> {
        if !self.first {
            self.out.write_all(b",")?;
        }
        self.first = false;
        self.out.write_all(b"\n{")?;
        let mut field_count = 0usize;
        self.field(
            "SourceFile",
            &json_string(&report.source.to_string_lossy()),
            &mut field_count,
        )?;
        let version_key = self.key("ExifTool", "ExifTool", "ExifToolVersion");
        let wants_version = self.opts.selection.is_empty()
            || self.opts.selection.iter().any(|selection| {
                selection == "EXIFTOOLVERSION" || selection == "EXIFTOOL:EXIFTOOLVERSION"
            });
        if wants_version {
            self.field(&version_key, REPORTED_EXIFTOOL_VERSION, &mut field_count)?;
        }

        if let Some(error) = &report.error {
            self.field("Error", &json_string(error), &mut field_count)?;
        }
        if !report.warnings.is_empty() {
            let warning_key = self.key("ExifTool", "ExifTool", "Warning");
            self.field(
                &warning_key,
                &json_string(&report.warnings.join("; ")),
                &mut field_count,
            )?;
        }

        let mut seen: HashSet<String> = HashSet::new();
        self.system_fields(&report.system, &mut field_count, &mut seen)?;

        for tag in &report.tags {
            if !self.opts.selects(tag.group0, tag.group1, tag.name) {
                continue;
            }
            let key = self.key(tag.group0, tag.group1, tag.name);
            if !seen.insert(key.clone()) {
                continue;
            }
            let value = self.tag_value(tag);
            self.field(&key, &value, &mut field_count)?;
        }

        self.out.write_all(b"\n}")?;
        Ok(())
    }

    fn system_fields(
        &mut self,
        system: &SystemFields,
        field_count: &mut usize,
        seen: &mut HashSet<String>,
    ) -> io::Result<()> {
        let file_size = if self.opts.numeric {
            system.file_size.to_string()
        } else {
            json_string(&crate::formatting::human_size(system.file_size))
        };
        let permissions = if self.opts.numeric {
            system.file_mode.to_string()
        } else {
            json_string(&system.file_permissions)
        };
        let entries: [(&str, &str, &str, String); 10] = [
            (
                "System",
                "System",
                "FileName",
                json_string(&system.file_name),
            ),
            (
                "System",
                "System",
                "Directory",
                json_string(&system.directory),
            ),
            ("System", "System", "FileSize", file_size),
            (
                "System",
                "System",
                "FileModifyDate",
                json_string(&system.file_modify_date),
            ),
            (
                "System",
                "System",
                "FileAccessDate",
                json_string(&system.file_access_date),
            ),
            (
                "System",
                "System",
                "FileInodeChangeDate",
                json_string(&system.file_inode_change_date),
            ),
            ("System", "System", "FilePermissions", permissions),
            ("File", "File", "FileType", json_string(&system.file_type)),
            (
                "File",
                "File",
                "FileTypeExtension",
                json_string(&system.file_type_extension),
            ),
            ("File", "File", "MIMEType", json_string(&system.mime_type)),
        ];

        for (group0, group1, name, value) in entries {
            if !self.opts.selects(group0, group1, name) {
                continue;
            }
            let key = self.key(group0, group1, name);
            if !seen.insert(key.clone()) {
                continue;
            }
            self.field(&key, &value, field_count)?;
        }
        Ok(())
    }

    fn key(&self, group0: &str, group1: &str, name: &str) -> String {
        match self.opts.group {
            GroupMode::None => name.to_string(),
            GroupMode::G0 => format!("{group0}:{name}"),
            GroupMode::G1 => format!("{group1}:{name}"),
        }
    }

    fn tag_value(&self, tag: &Tag) -> String {
        if !self.opts.numeric
            && let Some(print) = &tag.print
        {
            if let Ok(number) = print.parse::<f64>() {
                return format_float(number);
            }
            return json_string(print);
        }
        match &tag.value {
            Value::Str(text) => {
                if text == "True" {
                    return "true".to_string();
                }
                if text == "False" {
                    return "false".to_string();
                }
                if let Some(number) = numeric_string(text) {
                    return number;
                }
                json_string(text)
            }
            Value::Int(value) => value.to_string(),
            Value::Float(value) => format_float(*value),
            Value::Rational { .. } => {
                let display = tag.value.display();
                if display.contains('/') {
                    json_string(&display)
                } else {
                    display
                }
            }
            Value::Bytes(_) => json_string(&tag.value.display()),
        }
    }

    fn field(&mut self, key: &str, value: &str, count: &mut usize) -> io::Result<()> {
        if *count > 0 {
            self.out.write_all(b",")?;
        }
        *count += 1;
        self.out.write_all(b"\n  ")?;
        self.out.write_all(json_string(key).as_bytes())?;
        self.out.write_all(b": ")?;
        self.out.write_all(value.as_bytes())
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

/// Strings that look like plain decimal numbers become JSON numbers, unless
/// they carry significant leading zeros (`0232` must stay a string).
fn numeric_string(text: &str) -> Option<String> {
    let number: f64 = text.parse().ok()?;
    let unsigned = text.trim_start_matches(['-', '+']);
    if unsigned.len() > 1 && unsigned.starts_with('0') && !unsigned.starts_with("0.") {
        return None;
    }
    if text.starts_with('+') {
        return None;
    }
    Some(format_float(number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tag;
    use std::path::PathBuf;

    fn report() -> FileReport {
        let mut report = FileReport::new(PathBuf::from("/tmp/a.jpg"));
        report.system = SystemFields {
            file_name: "a.jpg".to_string(),
            directory: "/tmp".to_string(),
            file_size: 26_831,
            file_modify_date: "2026:11:21 15:30:56+03:00".to_string(),
            file_access_date: "2026:11:24 21:50:23+03:00".to_string(),
            file_inode_change_date: "2026:11:21 15:30:56+03:00".to_string(),
            file_permissions: "-rw-r--r--".to_string(),
            file_mode: 0o100_644,
            file_type: "JPEG".to_string(),
            file_type_extension: "jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
        };
        report.tags.push(Tag::new(
            "EXIF",
            "IFD0",
            "Make",
            Value::Str("Canon".to_string()),
        ));
        report
    }

    #[test]
    fn writes_two_records_in_order() {
        let mut buffer = Vec::new();
        let mut emitter = Emitter::new(&mut buffer, ReadOptions::default());
        emitter.begin().unwrap();
        emitter.record(&report()).unwrap();
        let mut second = report();
        second.source = PathBuf::from("/tmp/b.jpg");
        emitter.record(&second).unwrap();
        emitter.finish().unwrap();
        let text = String::from_utf8(buffer).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|err| panic!("{err}\n{text}"));
        let array = parsed.as_array().unwrap();
        assert_eq!(array.len(), 2);
        assert_eq!(array[0]["SourceFile"], "/tmp/a.jpg");
        assert_eq!(array[1]["SourceFile"], "/tmp/b.jpg");
        assert_eq!(array[0]["FileSize"], "27 kB");
        assert_eq!(array[0]["Make"], "Canon");
    }

    #[test]
    fn numeric_mode_changes_file_size_and_permissions() {
        let mut buffer = Vec::new();
        let mut opts = ReadOptions {
            numeric: true,
            ..Default::default()
        };
        opts.group = GroupMode::G1;
        let mut emitter = Emitter::new(&mut buffer, opts);
        emitter.begin().unwrap();
        emitter.record(&report()).unwrap();
        emitter.finish().unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buffer).unwrap();
        assert_eq!(parsed[0]["System:FileSize"], 26_831);
        assert_eq!(parsed[0]["System:FilePermissions"], 0o100_644);
    }

    #[test]
    fn escaping_keeps_non_ascii() {
        let mut buffer = Vec::new();
        let mut emitter = Emitter::new(&mut buffer, ReadOptions::default());
        emitter.begin().unwrap();
        let mut report = report();
        report.tags.push(Tag::new(
            "XMP",
            "XMP-dc",
            "Title",
            Value::Str("Вечер \"город\"".to_string()),
        ));
        emitter.record(&report).unwrap();
        emitter.finish().unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert!(text.contains("Вечер"));
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed[0]["Title"], "Вечер \"город\"");
    }
}
