//! Date writing MVP: in-place EXIF date values + FileModifyDate.
//!
//! Only existing date fields are rewritten; missing fields are reported instead
//! of being inserted. JPEG and TIFF-family containers are supported.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};

use crate::tiff::Tiff;

pub const WRITABLE: [&str; 4] = [
    "DATETIMEORIGINAL",
    "CREATEDATE",
    "MODIFYDATE",
    "FILEMODIFYDATE",
];

pub fn is_writable(name: &str) -> bool {
    let name = name.trim_start_matches('-').to_ascii_uppercase();
    WRITABLE.contains(&name.as_str()) || name == "ALLDATES"
}

pub fn normalize_date(value: &str) -> Result<String, String> {
    let value = value.trim();
    for format in ["%Y:%m:%d %H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(date) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(date.format("%Y:%m:%d %H:%M:%S").to_string());
        }
    }
    Err(format!(
        "invalid date {value:?}: expected YYYY:MM:DD HH:MM:SS"
    ))
}

/// Apply metadata dates and/or FileModifyDate.
pub fn write(path: &Path, assignments: &[(String, String)]) -> Result<usize, String> {
    let mut exif_updates = Vec::new();
    let mut file_modify = None;
    for (name, value) in assignments {
        let name = name.trim_start_matches('-').to_ascii_uppercase();
        let value = normalize_date(value)?;
        match name.as_str() {
            "FILEMODIFYDATE" => file_modify = Some(value),
            "ALLDATES" => {
                exif_updates.push(("DateTimeOriginal".to_string(), value.clone()));
                exif_updates.push(("CreateDate".to_string(), value.clone()));
                exif_updates.push(("ModifyDate".to_string(), value));
            }
            "DATETIMEORIGINAL" | "CREATEDATE" | "MODIFYDATE" => {
                exif_updates.push((title_case(&name), value));
            }
            _ => return Err(format!("unsupported write tag: {name}")),
        }
    }

    let mut written = 0usize;
    if !exif_updates.is_empty() {
        let data = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let mut patched = data.clone();
        apply_exif(&mut patched, &exif_updates)?;
        atomic_replace(path, &patched)?;
        written += exif_updates.len();
    }
    if let Some(value) = file_modify {
        set_file_modify(path, &value)?;
        written += 1;
    }
    Ok(written)
}

fn title_case(name: &str) -> String {
    match name {
        "DATETIMEORIGINAL" => "DateTimeOriginal".to_string(),
        "CREATEDATE" => "CreateDate".to_string(),
        "MODIFYDATE" => "ModifyDate".to_string(),
        other => other.to_string(),
    }
}

fn apply_exif(data: &mut [u8], updates: &[(String, String)]) -> Result<(), String> {
    let base = exif_base(data).ok_or_else(|| "no EXIF/TIFF payload found".to_string())?;
    let tiff = Tiff::at(data, base).ok_or_else(|| "invalid TIFF header".to_string())?;
    let ifd = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
        .ok_or_else(|| "cannot read IFD0".to_string())?;
    let exif_ifd = ifd
        .entries
        .iter()
        .find(|entry| entry.tag == 0x8769)
        .and_then(|entry| tiff.read_ifd(entry.value_or_offset));
    for (name, value) in updates {
        let (entry, _) = match name.as_str() {
            "DateTimeOriginal" => exif_ifd
                .as_ref()
                .and_then(|ifd| ifd.entries.iter().find(|entry| entry.tag == 0x9003))
                .map(|entry| (entry, 0x9003)),
            "CreateDate" => exif_ifd
                .as_ref()
                .and_then(|ifd| ifd.entries.iter().find(|entry| entry.tag == 0x9004))
                .map(|entry| (entry, 0x9004)),
            "ModifyDate" => ifd
                .entries
                .iter()
                .find(|entry| entry.tag == 0x0132)
                .map(|entry| (entry, 0x0132)),
            _ => None,
        }
        .ok_or_else(|| format!("{name} tag not found in file"))?;
        if entry.typ != 2 || entry.count < 20 {
            return Err(format!("{name} is not a writable ASCII date"));
        }
        let start = if entry.count <= 4 {
            base + entry.entry_offset + 8
        } else {
            base + entry.value_or_offset as usize
        };
        let end = start + 19;
        if end > data.len() {
            return Err(format!("{name} value is outside the file"));
        }
        data[start..end].copy_from_slice(value.as_bytes());
        data[end] = 0;
    }
    Ok(())
}

fn exif_base(data: &[u8]) -> Option<usize> {
    if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        return Some(0);
    }
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = data[pos + 1];
        if matches!(marker, 0xD8 | 0xD9 | 0x01 | 0xD0..=0xD7) {
            pos += 2;
            continue;
        }
        let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if length < 2 || pos + 2 + length > data.len() {
            break;
        }
        let payload = &data[pos + 4..pos + 2 + length];
        if marker == 0xE1 && payload.starts_with(b"Exif\0\0") {
            return Some(pos + 10);
        }
        pos += 2 + length;
    }
    None
}

fn atomic_replace(path: &Path, data: &[u8]) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    let temp = temp_path(path);
    {
        let mut file = File::create(&temp).map_err(|err| err.to_string())?;
        use std::io::Write;
        file.write_all(data).map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
    }
    fs::set_permissions(&temp, metadata.permissions()).map_err(|err| err.to_string())?;
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        err.to_string()
    })?;
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("etng");
    path.with_file_name(format!(".{name}.etng-{}.tmp", std::process::id()))
}

fn set_file_modify(path: &Path, value: &str) -> Result<(), String> {
    let date = NaiveDateTime::parse_from_str(value, "%Y:%m:%d %H:%M:%S")
        .map_err(|err| format!("invalid FileModifyDate: {err}"))?;
    let local = Local
        .from_local_datetime(&date)
        .single()
        .ok_or_else(|| "ambiguous local date".to_string())?;
    let file = File::options()
        .write(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.set_modified(local.into())
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date_tiff() -> Vec<u8> {
        let mut data = vec![0u8; 128];
        data[0..4].copy_from_slice(b"II*\0");
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        data[8..10].copy_from_slice(&2u16.to_le_bytes());
        let entry = |data: &mut [u8], at: usize, tag: u16, value: u32| {
            data[at..at + 2].copy_from_slice(&tag.to_le_bytes());
            data[at + 2..at + 4].copy_from_slice(&2u16.to_le_bytes());
            data[at + 4..at + 8].copy_from_slice(&20u32.to_le_bytes());
            data[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
        };
        entry(&mut data, 10, 0x0132, 80);
        data[22..34].copy_from_slice(&[0u8; 12]);
        data[46..50].copy_from_slice(&0u32.to_le_bytes());
        data[80..99].copy_from_slice(b"2020:01:01 00:00:00");
        data[99] = 0;
        data
    }

    #[test]
    fn rewrites_existing_modify_date() {
        let mut data = date_tiff();
        apply_exif(
            &mut data,
            &[("ModifyDate".to_string(), "2024:05:06 07:08:09".to_string())],
        )
        .unwrap();
        assert_eq!(&data[80..99], b"2024:05:06 07:08:09");
    }

    #[test]
    fn normalizes_date() {
        assert_eq!(
            normalize_date("2024-05-06 07:08:09").unwrap(),
            "2024:05:06 07:08:09"
        );
        assert!(normalize_date("yesterday").is_err());
    }
}
