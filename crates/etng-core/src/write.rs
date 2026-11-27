//! Date writing MVP: in-place EXIF date values + FileModifyDate.
//!
//! Only existing date fields are rewritten; missing fields are reported instead
//! of being inserted. JPEG and TIFF-family containers are supported.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};

use crate::date_sidecars;
use crate::sniff::{self, FileType};
use crate::tiff::Tiff;
use crate::{cr3, heic};

pub const WRITABLE: [&str; 4] = [
    "DATETIMEORIGINAL",
    "CREATEDATE",
    "MODIFYDATE",
    "FILEMODIFYDATE",
];

pub const WRITABLE_GPS: [&str; 6] = [
    "GPSLATITUDE",
    "GPSLATITUDEREF",
    "GPSLONGITUDE",
    "GPSLONGITUDEREF",
    "GPSALTITUDE",
    "GPSALTITUDEREF",
];

pub const WRITABLE_TEXT: [&str; 13] = [
    "CITY",
    "COUNTRY",
    "STATE",
    "LOCATION",
    "KEYWORDS",
    "CREATOR",
    "COPYRIGHT",
    "COUNTRYCODE",
    "WORLDREGION",
    "CODEDCHARSET",
    "CODEDCHARACTERSET",
    "IMAGEUNIQUEID",
    "COLLECTIONS",
];

pub fn is_writable(name: &str) -> bool {
    let name = name.trim_start_matches('-').to_ascii_uppercase();
    WRITABLE.contains(&name.as_str())
        || WRITABLE_TEXT.contains(&name.as_str())
        || WRITABLE_GPS.contains(&name.as_str())
        || name == "ALLDATES"
}

fn is_text(name: &str) -> bool {
    WRITABLE_TEXT.contains(&name.to_ascii_uppercase().as_str())
}

fn is_gps(name: &str) -> bool {
    WRITABLE_GPS.contains(&name.to_ascii_uppercase().as_str())
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
    let mut gps_updates = Vec::new();
    let mut file_modify = None;
    for (name, value) in assignments {
        let name = name.trim_start_matches('-').to_ascii_uppercase();
        if is_text(&name) {
            text_updates.push((title_case(&name), value.clone()));
            continue;
        }
        if is_gps(&name) {
            gps_updates.push((title_case(&name), value.clone()));
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
        let kind = sniff::detect(&data);
        if matches!(kind, FileType::Heic | FileType::Cr3 | FileType::Mov) {
            if !text_updates.is_empty() {
                return Err(format!(
                    "{}: text tags are not supported for {} yet",
                    path.display(),
                    kind.name()
                ));
            }
            let patched = patch_bmff_dates(&data, &exif_updates)?;
            atomic_replace(path, &patched)?;
            written += exif_updates.len();
        } else if matches!(
            kind,
            FileType::Jpeg
                | FileType::Tiff
                | FileType::Dng
                | FileType::Cr2
                | FileType::Arw
                | FileType::Nef
        ) {
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
        } else {
            return Err(format!(
                "{}: date writing is not supported for {}",
                path.display(),
                kind.name()
            ));
        }
    }
    if !text_updates.is_empty() || !gps_updates.is_empty() {
        let data = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let kind = sniff::detect(&data);
        if !matches!(
            kind,
            FileType::Jpeg
                | FileType::Tiff
                | FileType::Dng
                | FileType::Cr2
                | FileType::Arw
                | FileType::Nef
        ) {
            return Err(format!(
                "{}: GPS/text tags are not supported for {}",
                path.display(),
                kind.name()
            ));
        }
        let mut patched = apply_exif_text(&data, &text_updates)?;
        patched = apply_exif_gps(&patched, &gps_updates)?;
        if let Some(updated) = date_sidecars::update_text(&patched, &text_updates)? {
            atomic_replace(path, &updated)?;
        } else if patched != data {
            atomic_replace(path, &patched)?;
        }
        written += text_updates.len() + gps_updates.len();
    }
    if let Some(value) = file_modify {
        set_file_modify(path, &value)?;
        written += 1;
    }
    Ok(written)
}

fn patch_bmff_dates(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let kind = sniff::detect(data);
    let mut out = data.to_vec();
    if matches!(kind, FileType::Heic | FileType::Cr3) {
        let payloads: Vec<&[u8]> = match kind {
            FileType::Heic => heic::extract_exif(data).into_iter().collect(),
            FileType::Cr3 => cr3::exif_payloads(data),
            _ => Vec::new(),
        };
        if payloads.is_empty() {
            return Err(format!("{}: EXIF payload not found", kind.name()));
        }
        let ranges: Vec<(usize, usize)> = payloads
            .iter()
            .map(|payload| {
                (
                    payload.as_ptr() as usize - data.as_ptr() as usize,
                    payload.len(),
                )
            })
            .collect();
        for (name, value) in updates {
            let mut applied = false;
            for (offset, length) in &ranges {
                let mut tiff = out[*offset..*offset + *length].to_vec();
                if apply_exif(&mut tiff, &[(name.clone(), value.clone())]).is_ok() {
                    out[*offset..*offset + *length].copy_from_slice(&tiff);
                    applied = true;
                    break;
                }
            }
            if !applied {
                if kind == FileType::Heic {
                    return insert_heic_dates(data, updates);
                }
                if kind == FileType::Cr3 {
                    return insert_cr3_dates(data, updates);
                }
                return Err(format!("{}: {name} tag not found in EXIF", kind.name()));
            }
        }
    }
    if matches!(kind, FileType::Cr3 | FileType::Mov) {
        patch_movie_header_dates(&mut out, updates, kind == FileType::Cr3)?;
    }
    Ok(out)
}

fn insert_heic_dates(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let info = heic::exif_info(data).ok_or_else(|| "HEIC: EXIF item not found".to_string())?;
    let tiff = data
        .get(info.tiff_start..info.tiff_start + info.tiff_len)
        .ok_or_else(|| "HEIC: invalid EXIF range".to_string())?;
    let new_tiff = insert_dates(tiff, updates)?;
    let prefix = data
        .get(info.item_start..info.tiff_start)
        .ok_or_else(|| "HEIC: invalid EXIF prefix".to_string())?;
    let suffix = data
        .get(info.tiff_start + info.tiff_len..info.item_start + info.item_len)
        .ok_or_else(|| "HEIC: invalid EXIF suffix".to_string())?;
    let mut new_item = Vec::with_capacity(prefix.len() + new_tiff.len() + suffix.len());
    new_item.extend_from_slice(prefix);
    new_item.extend_from_slice(&new_tiff);
    new_item.extend_from_slice(suffix);
    let total = new_item.len() + 8;
    if total > u32::MAX as usize {
        return Err("HEIC: EXIF item too large".to_string());
    }
    let mut out = data.to_vec();
    let item_offset = out.len() + 8;
    out.extend_from_slice(&(total as u32).to_be_bytes());
    out.extend_from_slice(b"mdat");
    out.extend_from_slice(&new_item);
    heic::set_exif_location(
        &mut out,
        info.item_id,
        item_offset as u64,
        new_item.len() as u64,
    )?;
    Ok(out)
}

fn insert_cr3_dates(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut out = data.to_vec();
    let groups: [(&[u8; 4], &[&str]); 2] = [
        (b"CMT2", &["DateTimeOriginal", "CreateDate"]),
        (b"CMT1", &["ModifyDate"]),
    ];
    for (kind, names) in groups {
        let subset: Vec<(String, String)> = updates
            .iter()
            .filter(|(name, _)| names.contains(&name.as_str()))
            .cloned()
            .collect();
        if subset.is_empty() {
            continue;
        }
        let Some((start, size, header)) = find_box_any(&out, kind) else {
            continue;
        };
        let old_end = start + size;
        let payload = out
            .get(start + header..old_end)
            .ok_or_else(|| "CR3: invalid CMT payload".to_string())?;
        let new_payload = insert_dates(payload, &subset)?;
        if new_payload.len() == payload.len() {
            out[start + header..old_end].copy_from_slice(&new_payload);
            continue;
        }
        let new_size = header + new_payload.len();
        if new_size > u32::MAX as usize {
            return Err("CR3: CMT box too large".to_string());
        }
        let delta = new_size as isize - size as isize;
        out.splice(old_end..old_end, std::iter::repeat_n(0u8, delta as usize));
        out[start..start + 4].copy_from_slice(&(new_size as u32).to_be_bytes());
        out[start + header..start + new_size].copy_from_slice(&new_payload);
        let Some((moov_start, moov_size, moov_header)) = find_box(&out, b"moov") else {
            return Err("CR3: moov not found".to_string());
        };
        if moov_start <= start && start < moov_start + moov_size {
            let new_moov = (moov_size as isize + delta) as u32;
            out[moov_start..moov_start + 4].copy_from_slice(&new_moov.to_be_bytes());
            let _ = moov_header;
        }
        shift_chunk_offsets(&mut out, old_end, delta)?;
    }
    Ok(out)
}

fn shift_chunk_offsets(data: &mut [u8], insertion: usize, delta: isize) -> Result<(), String> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let kind = &data[pos + 4..pos + 8];
        if kind != b"stco" && kind != b"co64" {
            let size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                as usize;
            pos += if size >= 8 { size } else { 1 };
            continue;
        }
        let size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        if size < 16 || pos + size > data.len() {
            pos += 1;
            continue;
        }
        let count = u32::from_be_bytes([
            data[pos + 12],
            data[pos + 13],
            data[pos + 14],
            data[pos + 15],
        ]) as usize;
        let width = if kind == b"stco" { 4 } else { 8 };
        for index in 0..count {
            let at = pos + 16 + index * width;
            if at + width > pos + size {
                break;
            }
            let old = read_be(&data[at..at + width]);
            if old as usize >= insertion {
                let new = (old as isize + delta) as u64;
                write_be_bytes(&mut data[at..at + width], new)?;
            }
        }
        pos += size;
    }
    Ok(())
}

fn read_be(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0u64, |value, byte| (value << 8) | *byte as u64)
}

fn write_be_bytes(bytes: &mut [u8], value: u64) -> Result<(), String> {
    let width = bytes.len();
    if width > 8 {
        return Err("invalid offset width".to_string());
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (value >> (8 * (width - index - 1))) as u8;
    }
    Ok(())
}

fn patch_movie_header_dates(
    data: &mut [u8],
    updates: &[(String, String)],
    local_time: bool,
) -> Result<(), String> {
    let Some((moov_start, moov_size, moov_header)) = find_box(data, b"moov") else {
        return Ok(());
    };
    let moov_payload = &data[moov_start + moov_header..moov_start + moov_size];
    let Some((mvhd_start, mvhd_size, mvhd_header)) = find_box(moov_payload, b"mvhd") else {
        return Ok(());
    };
    let mvhd = moov_start + moov_header + mvhd_start + mvhd_header;
    let mvhd_end = moov_start + moov_header + mvhd_start + mvhd_size;
    if mvhd_end > data.len() || mvhd + 1 > data.len() {
        return Err("invalid mvhd box".to_string());
    }
    let version = data[mvhd];
    for (name, value) in updates {
        let field = match name.as_str() {
            "DateTimeOriginal" | "CreateDate" => 0,
            "ModifyDate" => 1,
            _ => continue,
        };
        let seconds = qt_seconds(value, local_time)?;
        if version == 1 {
            let at = mvhd + 4 + field * 8;
            if at + 8 > mvhd_end {
                return Err("invalid version-1 mvhd dates".to_string());
            }
            data[at..at + 8].copy_from_slice(&seconds.to_be_bytes());
        } else {
            let at = mvhd + 4 + field * 4;
            if at + 4 > mvhd_end {
                return Err("invalid version-0 mvhd dates".to_string());
            }
            let seconds = u32::try_from(seconds).map_err(|_| "date is out of range".to_string())?;
            data[at..at + 4].copy_from_slice(&seconds.to_be_bytes());
        }
    }
    Ok(())
}

fn qt_seconds(value: &str, local_time: bool) -> Result<u64, String> {
    let date = NaiveDateTime::parse_from_str(value, "%Y:%m:%d %H:%M:%S")
        .map_err(|_| format!("invalid QuickTime date {value}"))?;
    let timestamp = if local_time {
        let local = match Local.from_local_datetime(&date) {
            chrono::LocalResult::Single(local) => local,
            chrono::LocalResult::Ambiguous(first, _) => first,
            chrono::LocalResult::None => return Err(format!("invalid local date {value}")),
        };
        local.timestamp()
    } else {
        date.and_utc().timestamp()
    };
    let seconds = timestamp + 2_082_844_800;
    u64::try_from(seconds).map_err(|_| "date is before 1904".to_string())
}

fn find_box_any(data: &[u8], kind: &[u8; 4]) -> Option<(usize, usize, usize)> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        if &data[pos + 4..pos + 8] == kind {
            let size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                as usize;
            if size >= 8 && pos + size <= data.len() {
                return Some((pos, size, 8));
            }
        }
        pos += 1;
    }
    None
}

fn find_box(data: &[u8], kind: &[u8; 4]) -> Option<(usize, usize, usize)> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let current = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let (header, size) = if size32 == 1 {
            if pos + 16 > data.len() {
                break;
            }
            (
                16usize,
                u64::from_be_bytes(data[pos + 8..pos + 16].try_into().ok()?) as usize,
            )
        } else if size32 == 0 {
            (8usize, data.len().saturating_sub(pos))
        } else {
            (8usize, size32 as usize)
        };
        if size < header || pos + size > data.len() {
            break;
        }
        if &current == kind {
            return Some((pos, size, header));
        }
        pos += size;
    }
    None
}

fn set_exif_string(data: &[u8], tag: u16, value: &str) -> Result<Vec<u8>, String> {
    let bounds = exif_bounds(data).ok_or_else(|| "no EXIF/TIFF payload found".to_string())?;
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
    if let Some(entry) = exif_ifd.entries.iter().find(|entry| entry.tag == tag) {
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
    let total = exif_ifd.entries.len() + 1;
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
    for entry in &exif_ifd.entries {
        let start = entry.entry_offset;
        let raw = data
            .get(start..start + 12)
            .ok_or_else(|| "ExifIFD entry outside file".to_string())?;
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
    write_u32(&mut appended, 2 + total * 12, exif_ifd.next);
    let new_offset = aligned as u32;
    tiff_data.resize(aligned, 0);
    tiff_data.extend_from_slice(&appended);
    let pointer_at = pointer.entry_offset.saturating_sub(bounds.base) + 8;
    if pointer_at + 4 > tiff_data.len() {
        return Err("ExifIFD pointer outside TIFF".to_string());
    }
    write_u32(&mut tiff_data, pointer_at, new_offset);
    rebuild_tiff(data, bounds, tiff_data)
}

fn apply_exif_gps(data: &[u8], updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    if updates.is_empty() {
        return Ok(data.to_vec());
    }
    let bounds = exif_bounds(data).ok_or_else(|| "no EXIF/TIFF payload found".to_string())?;
    let tiff = Tiff::at(data, bounds.base).ok_or_else(|| "invalid TIFF header".to_string())?;
    let ifd0 = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
        .ok_or_else(|| "cannot read IFD0".to_string())?;
    let gps_pointer = ifd0.entries.iter().find(|entry| entry.tag == 0x8825);
    let old_gps = gps_pointer.and_then(|entry| tiff.read_ifd(entry.value_or_offset));
    let mut entries: Vec<(u16, u16, u32, Vec<u8>)> = Vec::new();
    if let Some(gps) = &old_gps {
        for entry in &gps.entries {
            if let Some(bytes) = tiff.entry_bytes(entry) {
                entries.push((entry.tag, entry.typ, entry.count, bytes.to_vec()));
            }
        }
    }
    for (name, value) in updates {
        let (tag, typ, count, bytes) = match name.as_str() {
            "GPSLatitude" => (0x0002, 5, 3, gps_rationals(value, tiff.le)?),
            "GPSLongitude" => (0x0004, 5, 3, gps_rationals(value, tiff.le)?),
            "GPSAltitude" => (0x0006, 5, 1, gps_altitude(value, tiff.le)?),
            "GPSLatitudeRef" => (0x0001, 2, 2, gps_ref(value, true)?),
            "GPSLongitudeRef" => (0x0003, 2, 2, gps_ref(value, false)?),
            "GPSAltitudeRef" => (0x0005, 1, 1, gps_altitude_ref(value)?),
            other => return Err(format!("unsupported GPS tag: {other}")),
        };
        if let Some(entry) = entries.iter_mut().find(|entry| entry.0 == tag) {
            *entry = (tag, typ, count, bytes);
        } else {
            entries.push((tag, typ, count, bytes));
        }
    }
    entries.sort_by_key(|entry| entry.0);
    let gps_offset = {
        let len = data[bounds.base..bounds.end].len();
        len + (len & 1)
    };
    let gps_bytes = build_ifd(&entries, gps_offset, 0, tiff.le);
    let mut tiff_data = data[bounds.base..bounds.end].to_vec();
    tiff_data.resize(gps_offset, 0);
    tiff_data.extend_from_slice(&gps_bytes);
    if let Some(pointer) = gps_pointer {
        let at = pointer.entry_offset.saturating_sub(bounds.base) + 8;
        write_u32_at(&mut tiff_data, at, gps_offset as u32, tiff.le)?;
    } else {
        let ifd0_offset = tiff_data.len() + (tiff_data.len() & 1);
        let mut raw_entries = Vec::new();
        for entry in &ifd0.entries {
            let start = entry.entry_offset;
            raw_entries.push(
                data.get(start..start + 12)
                    .ok_or_else(|| "IFD0 entry outside file".to_string())?
                    .to_vec(),
            );
        }
        let mut pointer = vec![0u8; 12];
        let tag_raw = if tiff.le {
            0x8825u16.to_le_bytes()
        } else {
            0x8825u16.to_be_bytes()
        };
        pointer[0..2].copy_from_slice(&tag_raw);
        let type_raw = if tiff.le {
            4u16.to_le_bytes()
        } else {
            4u16.to_be_bytes()
        };
        pointer[2..4].copy_from_slice(&type_raw);
        let count_raw = if tiff.le {
            1u32.to_le_bytes()
        } else {
            1u32.to_be_bytes()
        };
        pointer[4..8].copy_from_slice(&count_raw);
        let offset_raw = if tiff.le {
            (gps_offset as u32).to_le_bytes()
        } else {
            (gps_offset as u32).to_be_bytes()
        };
        pointer[8..12].copy_from_slice(&offset_raw);
        raw_entries.push(pointer);
        raw_entries.sort_by_key(|entry| u16::from_be_bytes([entry[0], entry[1]]));
        let mut ifd0_bytes = Vec::with_capacity(2 + raw_entries.len() * 12 + 4);
        ifd0_bytes.extend_from_slice(&(raw_entries.len() as u16).to_be_bytes());
        if tiff.le {
            ifd0_bytes[0..2].copy_from_slice(&(raw_entries.len() as u16).to_le_bytes());
        }
        for entry in &raw_entries {
            ifd0_bytes.extend_from_slice(entry);
        }
        let next_raw = if tiff.le {
            ifd0.next.to_le_bytes()
        } else {
            ifd0.next.to_be_bytes()
        };
        ifd0_bytes.extend_from_slice(&next_raw);
        tiff_data.resize(ifd0_offset, 0);
        tiff_data.extend_from_slice(&ifd0_bytes);
        let header = if tiff.le {
            (ifd0_offset as u32).to_le_bytes()
        } else {
            (ifd0_offset as u32).to_be_bytes()
        };
        tiff_data[4..8].copy_from_slice(&header);
    }
    rebuild_tiff(data, bounds, tiff_data)
}

fn write_u32_at(data: &mut [u8], at: usize, value: u32, le: bool) -> Result<(), String> {
    if at + 4 > data.len() {
        return Err("EXIF offset outside TIFF".to_string());
    }
    let raw = if le {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    data[at..at + 4].copy_from_slice(&raw);
    Ok(())
}

fn build_ifd(entries: &[(u16, u16, u32, Vec<u8>)], offset: usize, next: u32, le: bool) -> Vec<u8> {
    let count = entries.len();
    let values_start = offset + 2 + count * 12 + 4;
    let mut out = vec![0u8; 2 + count * 12 + 4];
    put_u16(&mut out, 0, count as u16, le);
    let mut values = Vec::new();
    for (index, (tag, typ, entry_count, bytes)) in entries.iter().enumerate() {
        let at = 2 + index * 12;
        put_u16(&mut out, at, *tag, le);
        put_u16(&mut out, at + 2, *typ, le);
        put_u32(&mut out, at + 4, *entry_count, le);
        if bytes.len() <= 4 {
            out[at + 8..at + 8 + bytes.len()].copy_from_slice(bytes);
        } else {
            let value_offset = values_start + values.len();
            put_u32(&mut out, at + 8, value_offset as u32, le);
            values.extend_from_slice(bytes);
            if values.len() & 1 == 1 {
                values.push(0);
            }
        }
    }
    put_u32(&mut out, 2 + count * 12, next, le);
    out.extend_from_slice(&values);
    out
}

fn put_u16(data: &mut [u8], at: usize, value: u16, le: bool) {
    let raw = if le {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    data[at..at + 2].copy_from_slice(&raw);
}

fn put_u32(data: &mut [u8], at: usize, value: u32, le: bool) {
    let raw = if le {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    data[at..at + 4].copy_from_slice(&raw);
}

fn gps_rationals(value: &str, le: bool) -> Result<Vec<u8>, String> {
    let cleaned = value.replace(['"', '\'', '°'], " ").replace("deg", " ");
    let mut parts: Vec<f64> = Vec::new();
    for part in cleaned.split(|ch: char| ch.is_whitespace() || ch == ',') {
        if part.is_empty() {
            continue;
        }
        parts.push(
            part.parse::<f64>()
                .map_err(|_| format!("invalid GPS coordinate {value}"))?,
        );
    }
    let (degrees, minutes, seconds) = match parts.as_slice() {
        [decimal] => {
            let abs = decimal.abs();
            let degrees = abs.floor();
            let minutes = ((abs - degrees) * 60.0).floor();
            let seconds = (abs - degrees - minutes / 60.0) * 3600.0;
            (degrees as u32, minutes as u32, seconds)
        }
        [degrees, minutes, seconds, ..] => (*degrees as u32, *minutes as u32, *seconds),
        _ => return Err(format!("invalid GPS coordinate {value}")),
    };
    let mut out = Vec::new();
    for (value, scale) in [
        (degrees as f64, 1u32),
        (minutes as f64, 1),
        (seconds, 10_000),
    ] {
        let numerator = (value * scale as f64).round() as u32;
        if le {
            out.extend_from_slice(&numerator.to_le_bytes());
            out.extend_from_slice(&scale.to_le_bytes());
        } else {
            out.extend_from_slice(&numerator.to_be_bytes());
            out.extend_from_slice(&scale.to_be_bytes());
        }
    }
    Ok(out)
}

fn gps_altitude(value: &str, le: bool) -> Result<Vec<u8>, String> {
    let meters = value
        .trim()
        .trim_end_matches('m')
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid GPS altitude {value}"))?
        .abs();
    let numerator = (meters * 1000.0).round() as u32;
    let mut out = Vec::new();
    if le {
        out.extend_from_slice(&numerator.to_le_bytes());
        out.extend_from_slice(&1000u32.to_le_bytes());
    } else {
        out.extend_from_slice(&numerator.to_be_bytes());
        out.extend_from_slice(&1000u32.to_be_bytes());
    }
    Ok(out)
}

fn gps_ref(value: &str, latitude: bool) -> Result<Vec<u8>, String> {
    let upper = value.trim().to_ascii_uppercase();
    let reference = match (latitude, upper.as_str()) {
        (true, "N" | "NORTH") => 'N',
        (true, "S" | "SOUTH") => 'S',
        (false, "E" | "EAST") => 'E',
        (false, "W" | "WEST") => 'W',
        _ => return Err(format!("invalid GPS reference {value}")),
    };
    Ok(vec![reference as u8, 0])
}

fn gps_altitude_ref(value: &str) -> Result<Vec<u8>, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "above sea level" | "above" | "sea level" => Ok(vec![0]),
        "1" | "below sea level" | "below" => Ok(vec![1]),
        _ => Err(format!("invalid GPS altitude reference {value}")),
    }
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
    if let Some((_, value)) = updates
        .iter()
        .rev()
        .find(|(name, _)| name == "ImageUniqueID")
    {
        out = set_exif_string(&out, 0xA420, value)?;
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
            Err(err) if err.contains("ExifIFD pointer not found") => {
                for (name, value) in &exif_updates {
                    let tag = match name.as_str() {
                        "DateTimeOriginal" => 0x9003,
                        "CreateDate" => 0x9004,
                        _ => continue,
                    };
                    patched = set_ifd0_string(&patched, tag, value)?;
                }
            }
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
        "COUNTRYCODE" => "CountryCode".to_string(),
        "WORLDREGION" => "WorldRegion".to_string(),
        "CODEDCHARSET" | "CODEDCHARACTERSET" => "CodedCharacterSet".to_string(),
        "IMAGEUNIQUEID" => "ImageUniqueID".to_string(),
        "COLLECTIONS" => "Collections".to_string(),
        "GPSLATITUDE" => "GPSLatitude".to_string(),
        "GPSLATITUDEREF" => "GPSLatitudeRef".to_string(),
        "GPSLONGITUDE" => "GPSLongitude".to_string(),
        "GPSLONGITUDEREF" => "GPSLongitudeRef".to_string(),
        "GPSALTITUDE" => "GPSAltitude".to_string(),
        "GPSALTITUDEREF" => "GPSAltitudeRef".to_string(),
        other => other.to_string(),
    }
}

fn find_ifd_entry<'a>(
    ifd: &'a crate::tiff::Ifd,
    exif_ifd: Option<&'a crate::tiff::Ifd>,
    tag: u16,
) -> Option<&'a crate::tiff::Entry> {
    ifd.entries
        .iter()
        .find(|entry| entry.tag == tag)
        .or_else(|| exif_ifd.and_then(|ifd| ifd.entries.iter().find(|entry| entry.tag == tag)))
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
            "DateTimeOriginal" => {
                find_ifd_entry(&ifd, exif_ifd.as_ref(), 0x9003).map(|entry| (entry, 0x9003))
            }
            "CreateDate" => {
                find_ifd_entry(&ifd, exif_ifd.as_ref(), 0x9004).map(|entry| (entry, 0x9004))
            }
            "ModifyDate" => {
                find_ifd_entry(&ifd, exif_ifd.as_ref(), 0x0132).map(|entry| (entry, 0x0132))
            }
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
