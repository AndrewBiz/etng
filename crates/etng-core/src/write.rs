//! Date writing MVP: in-place EXIF date values + FileModifyDate.
//!
//! Only existing date fields are rewritten; missing fields are reported instead
//! of being inserted. JPEG and TIFF-family containers are supported.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};

use crate::date_sidecars;
use crate::tiff::Tiff;

pub const WRITABLE: [&str; 4] = [
    "DATETIMEORIGINAL",
    "CREATEDATE",
    "MODIFYDATE",
    "FILEMODIFYDATE",
];

pub const WRITABLE_TEXT: [&str; 7] = [
    "CITY",
    "COUNTRY",
    "STATE",
    "LOCATION",
    "KEYWORDS",
    "CREATOR",
    "COPYRIGHT",
];

pub fn is_writable(name: &str) -> bool {
    let name = name.trim_start_matches('-').to_ascii_uppercase();
    WRITABLE.contains(&name.as_str())
        || WRITABLE_TEXT.contains(&name.as_str())
        || name == "ALLDATES"
}

fn is_text(name: &str) -> bool {
    WRITABLE_TEXT.contains(&name.to_ascii_uppercase().as_str())
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
    let mut text_updates = Vec::new();
    let mut file_modify = None;
    for (name, value) in assignments {
        let name = name.trim_start_matches('-').to_ascii_uppercase();
        if is_text(&name) {
            text_updates.push((title_case(&name), value.clone()));
            continue;
        }
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
        match apply_exif(&mut patched, &exif_updates) {
            Ok(()) => {}
            Err(err) if err.contains("not found") => {
                patched = insert_dates(&patched, &exif_updates)?;
            }
            Err(err) => return Err(err),
        }
        if let Some(updated) = date_sidecars::update(&patched, &exif_updates)? {
            patched = updated;
        }
        atomic_replace(path, &patched)?;
        written += exif_updates.len();
    }
    if !text_updates.is_empty() {
        let data = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let patched = apply_exif_text(&data, &text_updates)?;
        if let Some(updated) = date_sidecars::update_text(&patched, &text_updates)? {
            atomic_replace(path, &updated)?;
        } else if patched != data {
            atomic_replace(path, &patched)?;
        }
        written += text_updates.len();
    }
    if let Some(value) = file_modify {
        set_file_modify(path, &value)?;
        written += 1;
    }
    Ok(written)
}

fn apply_exif_text(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut out = data.to_vec();
    let creators: Vec<&str> = updates
        .iter()
        .filter(|(name, _)| name == "Creator")
        .map(|(_, value)| value.as_str())
        .collect();
    if !creators.is_empty() {
        out = set_ifd0_string(&out, 0x013B, &creators.join("; "))?;
    }
    if let Some((_, value)) = updates.iter().rev().find(|(name, _)| name == "Copyright") {
        out = set_ifd0_string(&out, 0x8298, value)?;
    }
    Ok(out)
}

fn set_ifd0_string(data: &[u8], tag: u16, value: &str) -> Result<Vec<u8>, String> {
    let bounds = exif_bounds(data).ok_or_else(|| "no EXIF/TIFF payload found".to_string())?;
    let tiff = Tiff::at(data, bounds.base).ok_or_else(|| "invalid TIFF header".to_string())?;
    let ifd = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
        .ok_or_else(|| "cannot read IFD0".to_string())?;
    let bytes = value.as_bytes();
    let count = bytes.len() + 1;
    let mut tiff_data = data[bounds.base..bounds.end].to_vec();
    let write_u32 = |buffer: &mut [u8], at: usize, number: u32| {
        let raw = if tiff.le {
            number.to_le_bytes()
        } else {
            number.to_be_bytes()
        };
        buffer[at..at + 4].copy_from_slice(&raw);
    };
    if let Some(entry) = ifd.entries.iter().find(|entry| entry.tag == tag) {
        let entry_at = entry.entry_offset.saturating_sub(bounds.base);
        let old_count = entry.count as usize;
        if count <= old_count
            && (count <= 4 || entry.value_or_offset as usize + count <= tiff_data.len())
        {
            if count <= 4 {
                tiff_data[entry_at + 8..entry_at + 12].fill(0);
                tiff_data[entry_at + 8..entry_at + 8 + bytes.len()].copy_from_slice(bytes);
            } else {
                let at = entry.value_or_offset as usize;
                tiff_data[at..at + bytes.len()].copy_from_slice(bytes);
                tiff_data[at + bytes.len()] = 0;
            }
            write_u32(&mut tiff_data, entry_at + 4, count as u32);
            return rebuild_tiff(data, bounds, tiff_data);
        }
        let aligned = tiff_data.len() + (tiff_data.len() & 1);
        let at = aligned;
        tiff_data.resize(aligned, 0);
        tiff_data.extend_from_slice(bytes);
        tiff_data.push(0);
        write_u32(&mut tiff_data, entry_at + 4, count as u32);
        write_u32(&mut tiff_data, entry_at + 8, at as u32);
        return rebuild_tiff(data, bounds, tiff_data);
    }

    let aligned = tiff_data.len() + (tiff_data.len() & 1);
    let total = ifd.entries.len() + 1;
    let values_start = aligned + 2 + total * 12 + 4;
    let mut appended = vec![0u8; values_start + count - aligned];
    let write_u16 = |buffer: &mut [u8], at: usize, number: u16| {
        let raw = if tiff.le {
            number.to_le_bytes()
        } else {
            number.to_be_bytes()
        };
        buffer[at..at + 2].copy_from_slice(&raw);
    };
    write_u16(&mut appended, 0, total as u16);
    let mut entries = Vec::new();
    for entry in &ifd.entries {
        let start = entry.entry_offset;
        let raw = data
            .get(start..start + 12)
            .ok_or_else(|| "IFD0 entry outside file".to_string())?;
        entries.push((entry.tag, raw.to_vec()));
    }
    let mut entry = vec![0u8; 12];
    write_u16(&mut entry, 0, tag);
    write_u16(&mut entry, 2, 2);
    write_u32(&mut entry, 4, count as u32);
    write_u32(&mut entry, 8, values_start as u32);
    appended[values_start - aligned..values_start - aligned + bytes.len()].copy_from_slice(bytes);
    appended[values_start - aligned + bytes.len()] = 0;
    entries.push((tag, entry));
    entries.sort_by_key(|(tag, _)| *tag);
    for (index, (_, raw)) in entries.iter().enumerate() {
        let at = 2 + index * 12;
        appended[at..at + 12].copy_from_slice(raw);
    }
    write_u32(&mut appended, 2 + total * 12, ifd.next);
    let new_offset = aligned as u32;
    tiff_data.resize(aligned, 0);
    tiff_data.extend_from_slice(&appended);
    let offset_raw = if tiff.le {
        new_offset.to_le_bytes()
    } else {
        new_offset.to_be_bytes()
    };
    tiff_data[4..8].copy_from_slice(&offset_raw);
    rebuild_tiff(data, bounds, tiff_data)
}

fn rebuild_tiff(data: &[u8], bounds: Bounds, tiff_data: Vec<u8>) -> Result<Vec<u8>, String> {
    if let Some((marker, length)) = bounds.segment {
        let new_length = 2 + 6 + tiff_data.len();
        if new_length > u16::MAX as usize {
            return Err("EXIF segment too large after text update".to_string());
        }
        let mut out = Vec::with_capacity(data.len() + tiff_data.len());
        out.extend_from_slice(&data[..marker + 2]);
        out.extend_from_slice(&(new_length as u16).to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(&tiff_data);
        out.extend_from_slice(&data[marker + 2 + length..]);
        Ok(out)
    } else {
        Ok(tiff_data)
    }
}

fn insert_dates(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut patched = data.to_vec();
    let exif_updates: Vec<(String, String)> = updates
        .iter()
        .filter(|(name, _)| name == "DateTimeOriginal" || name == "CreateDate")
        .cloned()
        .collect();
    if !exif_updates.is_empty() {
        match insert_exif_dates(&patched, &exif_updates) {
            Ok(value) => patched = value,
            Err(err) if err.contains("no missing ExifIFD dates") => {}
            Err(err) => return Err(err),
        }
    }
    if let Some((_, value)) = updates.iter().find(|(name, _)| name == "ModifyDate") {
        patched = set_modify_date(&patched, value)?;
    }
    Ok(patched)
}

fn set_modify_date(data: &[u8], value: &str) -> Result<Vec<u8>, String> {
    let bounds = exif_bounds(data).ok_or_else(|| "no EXIF/TIFF payload found".to_string())?;
    let tiff = Tiff::at(data, bounds.base).ok_or_else(|| "invalid TIFF header".to_string())?;
    let ifd = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
        .ok_or_else(|| "cannot read IFD0".to_string())?;
    if let Some(entry) = ifd.entries.iter().find(|entry| entry.tag == 0x0132) {
        if entry.count < 20 {
            return Err("ModifyDate is not a writable ASCII date".to_string());
        }
        let start = if entry.count <= 4 {
            entry.entry_offset + 8
        } else {
            bounds.base + entry.value_or_offset as usize
        };
        if start + 19 > data.len() {
            return Err("ModifyDate value is outside the file".to_string());
        }
        let mut out = data.to_vec();
        out[start..start + 19].copy_from_slice(value.as_bytes());
        out[start + 19] = 0;
        return Ok(out);
    }
    let old_tiff = &data[bounds.base..bounds.end];
    let aligned = old_tiff.len() + (old_tiff.len() & 1);
    let count = ifd.entries.len() + 1;
    let values_start = aligned + 2 + count * 12 + 4;
    let mut appended = vec![0u8; values_start + 20 - aligned];
    let write_u16 = |buffer: &mut [u8], at: usize, number: u16, le: bool| {
        let raw = if le {
            number.to_le_bytes()
        } else {
            number.to_be_bytes()
        };
        buffer[at..at + 2].copy_from_slice(&raw);
    };
    let write_u32 = |buffer: &mut [u8], at: usize, number: u32, le: bool| {
        let raw = if le {
            number.to_le_bytes()
        } else {
            number.to_be_bytes()
        };
        buffer[at..at + 4].copy_from_slice(&raw);
    };
    write_u16(&mut appended, 0, count as u16, tiff.le);
    let mut entries = Vec::new();
    for entry in &ifd.entries {
        let start = entry.entry_offset;
        let raw = data
            .get(start..start + 12)
            .ok_or_else(|| "IFD0 entry outside file".to_string())?;
        entries.push((entry.tag, raw.to_vec()));
    }
    let mut entry = vec![0u8; 12];
    write_u16(&mut entry, 0, 0x0132, tiff.le);
    write_u16(&mut entry, 2, 2, tiff.le);
    write_u32(&mut entry, 4, 20, tiff.le);
    write_u32(&mut entry, 8, values_start as u32, tiff.le);
    appended[values_start - aligned..values_start - aligned + 19].copy_from_slice(value.as_bytes());
    entries.push((0x0132, entry));
    entries.sort_by_key(|(tag, _)| *tag);
    for (index, (_, raw)) in entries.iter().enumerate() {
        let at = 2 + index * 12;
        appended[at..at + 12].copy_from_slice(raw);
    }
    let next_at = 2 + count * 12;
    write_u32(&mut appended, next_at, ifd.next, tiff.le);

    let new_offset = aligned as u32;
    let mut out = old_tiff.to_vec();
    out.resize(aligned, 0);
    out.extend_from_slice(&appended);
    let offset_raw = if tiff.le {
        new_offset.to_le_bytes()
    } else {
        new_offset.to_be_bytes()
    };
    out[4..8].copy_from_slice(&offset_raw);
    if let Some((marker, length)) = bounds.segment {
        let new_length = 2 + 6 + out.len();
        if new_length > u16::MAX as usize {
            return Err("EXIF segment too large after insertion".to_string());
        }
        let mut file = Vec::with_capacity(data.len() + appended.len());
        file.extend_from_slice(&data[..marker + 2]);
        file.extend_from_slice(&(new_length as u16).to_be_bytes());
        file.extend_from_slice(b"Exif\0\0");
        file.extend_from_slice(&out);
        file.extend_from_slice(&data[marker + 2 + length..]);
        Ok(file)
    } else {
        Ok(out)
    }
}

fn insert_exif_dates(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let Some(bounds) = exif_bounds(data) else {
        return Err("no EXIF/TIFF payload found".to_string());
    };
    let tiff = Tiff::at(data, bounds.base).ok_or_else(|| "invalid TIFF header".to_string())?;
    let ifd0 = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
        .ok_or_else(|| "cannot read IFD0".to_string())?;
    let pointer = ifd0
        .entries
        .iter()
        .find(|entry| entry.tag == 0x8769)
        .ok_or_else(|| "ExifIFD pointer not found".to_string())?;
    let exif_ifd = tiff
        .read_ifd(pointer.value_or_offset)
        .ok_or_else(|| "cannot read ExifIFD".to_string())?;
    let mut entries = Vec::new();
    for entry in &exif_ifd.entries {
        let start = entry.entry_offset;
        let raw = data
            .get(start..start + 12)
            .ok_or_else(|| "ExifIFD entry outside file".to_string())?;
        entries.push((entry.tag, raw.to_vec()));
    }
    let mut added = Vec::new();
    for (name, value) in updates {
        let tag = match name.as_str() {
            "DateTimeOriginal" => 0x9003,
            "CreateDate" => 0x9004,
            "ModifyDate" => {
                return Err("ModifyDate insertion is not supported; tag is missing".to_string());
            }
            other => return Err(format!("unsupported insertion tag: {other}")),
        };
        if entries.iter().any(|(existing, _)| *existing == tag) {
            continue;
        }
        added.push((tag, value.clone()));
    }
    if added.is_empty() {
        return Err("no missing ExifIFD dates to insert".to_string());
    }
    let mut tiff_data = data[bounds.base..bounds.end].to_vec();
    for (name, value) in updates {
        let tag = match name.as_str() {
            "DateTimeOriginal" => 0x9003,
            "CreateDate" => 0x9004,
            "ModifyDate" => 0x0132,
            _ => continue,
        };
        if let Some(entry) = exif_ifd.entries.iter().find(|entry| entry.tag == tag) {
            let start = if entry.count <= 4 {
                entry.entry_offset.saturating_sub(bounds.base) + 8
            } else {
                entry.value_or_offset as usize
            };
            if start + 19 <= tiff_data.len() {
                tiff_data[start..start + 19].copy_from_slice(value.as_bytes());
                tiff_data[start + 19] = 0;
            }
        }
    }
    let aligned = tiff_data.len() + (tiff_data.len() & 1);
    let count = entries.len() + added.len();
    let values_start = aligned + 2 + count * 12 + 4;
    let mut appended = vec![0u8; values_start + added.len() * 20 - aligned];
    let write_u16 = |buffer: &mut [u8], at: usize, value: u16, le: bool| {
        let raw = if le {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        buffer[at..at + 2].copy_from_slice(&raw);
    };
    let write_u32 = |buffer: &mut [u8], at: usize, value: u32, le: bool| {
        let raw = if le {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        buffer[at..at + 4].copy_from_slice(&raw);
    };
    write_u16(&mut appended, 0, count as u16, tiff.le);
    let mut all: Vec<(u16, Vec<u8>)> = entries;
    for (index, (tag, value)) in added.iter().enumerate() {
        let offset = values_start + index * 20;
        let mut entry = vec![0u8; 12];
        write_u16(&mut entry, 0, *tag, tiff.le);
        write_u16(&mut entry, 2, 2, tiff.le);
        write_u32(&mut entry, 4, 20, tiff.le);
        write_u32(&mut entry, 8, offset as u32, tiff.le);
        let string_at = values_start + index * 20 - aligned;
        appended[string_at..string_at + 19].copy_from_slice(value.as_bytes());
        all.push((*tag, entry));
    }
    all.sort_by_key(|(tag, _)| *tag);
    for (index, (_, entry)) in all.iter().enumerate() {
        let at = 2 + index * 12;
        appended[at..at + 12].copy_from_slice(entry);
    }
    let next_at = 2 + count * 12;
    write_u32(&mut appended, next_at, exif_ifd.next, tiff.le);

    let new_offset = aligned as u32;
    tiff_data.resize(aligned, 0);
    tiff_data.extend_from_slice(&appended);
    let pointer_at = pointer.entry_offset.saturating_sub(bounds.base) + 8;
    if pointer_at + 4 > tiff_data.len() {
        return Err("ExifIFD pointer outside TIFF".to_string());
    }
    let raw = if tiff.le {
        new_offset.to_le_bytes()
    } else {
        new_offset.to_be_bytes()
    };
    tiff_data[pointer_at..pointer_at + 4].copy_from_slice(&raw);

    if let Some((marker, length)) = bounds.segment {
        let new_length = 2 + 6 + tiff_data.len();
        if new_length > u16::MAX as usize {
            return Err("EXIF segment too large after insertion".to_string());
        }
        let mut out = Vec::with_capacity(data.len() + appended.len());
        out.extend_from_slice(&data[..marker + 2]);
        out.extend_from_slice(&(new_length as u16).to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(&tiff_data);
        out.extend_from_slice(&data[marker + 2 + length..]);
        return Ok(out);
    }
    Ok(tiff_data)
}

fn title_case(name: &str) -> String {
    match name {
        "DATETIMEORIGINAL" => "DateTimeOriginal".to_string(),
        "CREATEDATE" => "CreateDate".to_string(),
        "MODIFYDATE" => "ModifyDate".to_string(),
        "CITY" => "City".to_string(),
        "COUNTRY" => "Country".to_string(),
        "STATE" => "State".to_string(),
        "LOCATION" => "Location".to_string(),
        "KEYWORDS" => "Keywords".to_string(),
        "CREATOR" => "Creator".to_string(),
        "COPYRIGHT" => "Copyright".to_string(),
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
            entry.entry_offset + 8
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

struct Bounds {
    base: usize,
    end: usize,
    segment: Option<(usize, usize)>,
}

fn exif_bounds(data: &[u8]) -> Option<Bounds> {
    if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        return Some(Bounds {
            base: 0,
            end: data.len(),
            segment: None,
        });
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
            return Some(Bounds {
                base: pos + 10,
                end: pos + 2 + length,
                segment: Some((pos, length)),
            });
        }
        pos += 2 + length;
    }
    None
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
