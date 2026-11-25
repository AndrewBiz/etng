//! Delegation to the system `exiftool`.

use std::path::{Path, PathBuf};
use std::process::Command;

use etng_core::{FileReport, ReadOptions, Tag, Value};

pub struct Fallback {
    available: Option<bool>,
}

impl Fallback {
    pub fn new() -> Self {
        Self { available: None }
    }

    pub fn available(&mut self) -> bool {
        if let Some(value) = self.available {
            return value;
        }
        let available = Command::new("exiftool")
            .arg("-ver")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        self.available = Some(available);
        available
    }

    /// Ask exiftool for one file and map its JSON into our model.
    pub fn delegate(&mut self, path: &Path, opts: &ReadOptions) -> Result<FileReport, String> {
        if !self.available() {
            return Err("exiftool not found in PATH".to_string());
        }
        let mut command = Command::new("exiftool");
        command.arg("-j").arg("-G1");
        if opts.numeric {
            command.arg("-n");
        }
        if opts.quiet {
            command.arg("-q");
        }
        for selection in &opts.selection {
            command.arg(format!("-{selection}"));
        }
        command.arg(path);
        let output = command
            .output()
            .map_err(|err| format!("failed to run exiftool: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "exiftool exited with {}",
                output.status.code().unwrap_or(-1)
            ));
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|err| format!("invalid exiftool JSON: {err}"))?;
        let entry = json
            .as_array()
            .and_then(|array| array.first())
            .ok_or_else(|| "exiftool returned no records".to_string())?;
        map_entry(path, entry)
    }
}

fn map_entry(path: &Path, entry: &serde_json::Value) -> Result<FileReport, String> {
    let object = entry
        .as_object()
        .ok_or_else(|| "exiftool record is not an object".to_string())?;
    if let Some(error) = object.get("Error").and_then(|v| v.as_str()) {
        return Err(error.to_string());
    }
    let mut report = FileReport::new(PathBuf::from(path));
    for (key, value) in object {
        if key == "SourceFile" || key == "ExifToolVersion" {
            continue;
        }
        let (prefix, name) = match key.split_once(':') {
            Some((prefix, name)) => (prefix, name),
            None => ("ExifTool", key.as_str()),
        };
        if prefix == "System" || prefix == "ExifTool" {
            continue;
        }
        if prefix == "File" {
            match name {
                "FileType" => {
                    if let Some(text) = value.as_str() {
                        report.system.file_type = text.to_string();
                    }
                }
                "FileTypeExtension" => {
                    if let Some(text) = value.as_str() {
                        report.system.file_type_extension = text.to_string();
                    }
                }
                "MIMEType" => {
                    if let Some(text) = value.as_str() {
                        report.system.mime_type = text.to_string();
                    }
                }
                "FileSize" => {}
                _ => report.tags.push(Tag::new(
                    "File",
                    "File",
                    leak(name),
                    json_to_value(value).unwrap_or(Value::Str(String::new())),
                )),
            }
            continue;
        }
        let group0 = match prefix {
            "IFD0" | "IFD1" | "ExifIFD" | "GPS" | "InteropIFD" => "EXIF",
            "IPTC" => "IPTC",
            "JFIF" | "JFXX" => "JFIF",
            "Composite" => "Composite",
            "Canon" | "Nikon" | "Sony" | "MakerNotes" => "MakerNotes",
            other if other.starts_with("XMP") => "XMP",
            other => other,
        };
        let group1 = match prefix {
            "ExifTool" => continue,
            other => other,
        };
        let Some(value) = json_to_value(value) else {
            continue;
        };
        report
            .tags
            .push(Tag::new(leak(group0), leak(group1), leak(name), value));
    }
    Ok(report)
}

fn json_to_value(value: &serde_json::Value) -> Option<Value> {
    Some(match value {
        serde_json::Value::String(text) => Value::Str(text.clone()),
        serde_json::Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                Value::Int(int)
            } else {
                Value::Float(number.as_f64()?)
            }
        }
        serde_json::Value::Bool(flag) => Value::Str(flag.to_string()),
        serde_json::Value::Array(items) => Value::Str(
            items
                .iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(text) => Some(text.clone()),
                    serde_json::Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => return None,
    })
}

/// Merge delegate tags into our own result, keeping our values on conflicts.
pub fn merge(own: &mut FileReport, delegate: FileReport) {
    for tag in delegate.tags {
        if own
            .tags
            .iter()
            .any(|existing| existing.group1 == tag.group1 && existing.name == tag.name)
        {
            continue;
        }
        own.tags.push(tag);
    }
    own.warnings.extend(delegate.warnings);
}

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}
