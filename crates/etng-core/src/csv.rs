//! CSV writer mirroring `exiftool -csv`: one row per file, stable columns.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::base64;
use crate::formatting::{format_float, numeric_string};
use crate::model::{FileReport, Tag, Value};
use crate::options::{GroupMode, ReadOptions};
use crate::system::system_fields;

pub fn key(opts: &ReadOptions, group0: &str, group1: &str, name: &str) -> String {
    match opts.group {
        GroupMode::None => name.to_string(),
        GroupMode::G0 => format!("{group0}:{name}"),
        GroupMode::G1 => format!("{group1}:{name}"),
    }
}

pub fn write_header<W: Write>(out: &mut W, columns: &[String]) -> io::Result<()> {
    out.write_all(b"SourceFile")?;
    for column in columns {
        out.write_all(b",")?;
        write_cell(out, column)?;
    }
    out.write_all(b"\n")
}

pub fn write_record<W: Write>(
    out: &mut W,
    report: &FileReport,
    columns: &[String],
    opts: &ReadOptions,
) -> io::Result<()> {
    let cells = cells(report, opts);
    write_cell(out, &report.source.to_string_lossy())?;
    for column in columns {
        out.write_all(b",")?;
        if let Some(value) = cells.get(column) {
            write_cell(out, value)?;
        }
    }
    out.write_all(b"\n")
}

/// Build the emitted key/value map for one file (deduplicated, filtered).
///
/// Without `-G`, ExifTool resolves duplicate names by group priority
/// (Composite > EXIF IFDs > XMP > IPTC > vendor tables); mirror that so
/// `City` comes from XMP and `GPSLatitude` from the Composite tag.
pub fn cells(report: &FileReport, opts: &ReadOptions) -> HashMap<String, String> {
    let mut chosen: HashMap<String, (u8, String)> = HashMap::new();
    for field in system_fields(&report.system, opts.numeric, opts.binary) {
        if opts.wants(field.group0, field.group1, field.name) {
            insert_cell(
                &mut chosen,
                key(opts, field.group0, field.group1, field.name),
                1,
                field.value,
            );
        }
    }
    for tag in &report.tags {
        if !opts.wants(tag.group0, tag.group1, tag.name) {
            continue;
        }
        insert_cell(
            &mut chosen,
            key(opts, tag.group0, tag.group1, tag.name),
            priority(tag.group1),
            cell_value(tag, opts),
        );
    }
    if !opts.selection.is_empty() && opts.wants("ExifTool", "ExifTool", "ExifToolVersion") {
        chosen.insert(
            key(opts, "ExifTool", "ExifTool", "ExifToolVersion"),
            (1, crate::compat::REPORTED_EXIFTOOL_VERSION.to_string()),
        );
    }
    chosen
        .into_iter()
        .map(|(key, (_, value))| (key, value))
        .collect()
}

fn insert_cell(
    chosen: &mut HashMap<String, (u8, String)>,
    key: String,
    priority: u8,
    value: String,
) {
    match chosen.get(&key) {
        Some((existing, _)) if *existing <= priority => {}
        _ => {
            chosen.insert(key, (priority, value));
        }
    }
}

fn priority(group1: &str) -> u8 {
    if group1 == "Composite" {
        0
    } else if matches!(group1, "IFD0" | "IFD1" | "ExifIFD" | "GPS" | "InteropIFD") {
        1
    } else if group1.starts_with("XMP") {
        2
    } else if group1 == "IPTC" {
        3
    } else {
        4
    }
}

pub fn cell_value(tag: &Tag, opts: &ReadOptions) -> String {
    if let Value::Bytes(bytes) = &tag.value {
        if opts.binary {
            return format!("base64:{}", base64::encode(bytes));
        }
        return tag.value.display();
    }
    if !opts.numeric
        && let Some(print) = &tag.print
    {
        return print.clone();
    }
    match &tag.value {
        Value::Str(text) => {
            if text == "True" {
                return "true".to_string();
            }
            if text == "False" {
                return "false".to_string();
            }
            numeric_string(text).unwrap_or_else(|| text.clone())
        }
        Value::Int(value) => value.to_string(),
        Value::Float(value) => format_float(*value),
        Value::Rational { .. } => tag.value.display(),
        Value::Bytes(_) => unreachable!("binary values return before the scalar match"),
    }
}

fn write_cell<W: Write>(out: &mut W, value: &str) -> io::Result<()> {
    let needs_quotes = value
        .bytes()
        .any(|byte| matches!(byte, b',' | b'"' | b'\n' | b'\r'));
    if needs_quotes {
        out.write_all(b"\"")?;
        out.write_all(value.replace('"', "\"\"").as_bytes())?;
        out.write_all(b"\"")
    } else {
        out.write_all(value.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SystemFields, Tag};

    fn report() -> FileReport {
        let mut report = FileReport::new(std::path::PathBuf::from("/tmp/a.jpg"));
        report.system = SystemFields {
            file_name: "a.jpg".to_string(),
            directory: "/tmp".to_string(),
            file_size: 4393,
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
        report.tags.push(Tag::new(
            "EXIF",
            "IFD0",
            "ImageDescription",
            Value::Str("On, \"Fired\"".to_string()),
        ));
        report
    }

    #[test]
    fn writes_header_and_escaped_row() {
        let mut buffer = Vec::new();
        let columns = vec!["ImageDescription".to_string(), "Make".to_string()];
        write_header(&mut buffer, &columns).unwrap();
        write_record(&mut buffer, &report(), &columns, &ReadOptions::default()).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert_eq!(
            text.lines().next(),
            Some("SourceFile,ImageDescription,Make")
        );
        assert!(text.contains("\"On, \"\"Fired\"\"\",Canon"));
    }

    #[test]
    fn binary_values_use_base64_in_binary_mode() {
        let mut report = report();
        report.tags.push(Tag::new(
            "EXIF",
            "IFD1",
            "ThumbnailImage",
            Value::Bytes(vec![0xFF, 0xD8, 0xFF]),
        ));
        let mut buffer = Vec::new();
        let columns = vec!["ThumbnailImage".to_string()];
        let opts = ReadOptions {
            binary: true,
            ..Default::default()
        };
        write_header(&mut buffer, &columns).unwrap();
        write_record(&mut buffer, &report, &columns, &opts).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert!(text.contains(",base64:/9j/"));
    }
}
