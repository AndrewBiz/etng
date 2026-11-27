//! PNG chunks: IHDR, eXIf, sRGB and tIME.

use crate::exif;
use crate::model::{Tag, Value};

fn itxt(payload: &[u8]) -> Option<(String, String)> {
    let key_end = payload.iter().position(|byte| *byte == 0)?;
    let keyword = String::from_utf8_lossy(&payload[..key_end]).to_string();
    if payload.get(key_end + 1).copied().unwrap_or(0) != 0 {
        return None;
    }
    let after = &payload.get(key_end + 3..)?;
    let language_end = after.iter().position(|byte| *byte == 0)?;
    let translated = &after.get(language_end + 1..)?;
    let translated_end = translated.iter().position(|byte| *byte == 0)?;
    let text = translated.get(translated_end + 1..)?;
    Some((
        keyword,
        String::from_utf8_lossy(text)
            .trim_end_matches('\0')
            .to_string(),
    ))
}

fn text_chunk(payload: &[u8]) -> Option<(String, String)> {
    let key_end = payload.iter().position(|byte| *byte == 0)?;
    Some((
        String::from_utf8_lossy(&payload[..key_end]).to_string(),
        String::from_utf8_lossy(&payload[key_end + 1..]).to_string(),
    ))
}

pub fn parse(data: &[u8], out: &mut Vec<Tag>, warnings: &mut Vec<String>) -> Option<(u32, u32)> {
    if !data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return None;
    }
    let mut pos = 8usize;
    let mut dimensions = None;
    while pos + 12 <= data.len() {
        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind = &data[pos + 4..pos + 8];
        let start = pos + 8;
        let end = start.checked_add(length)?;
        if end + 4 > data.len() {
            warnings.push("PNG: truncated chunk".to_string());
            break;
        }
        let payload = &data[start..end];
        match kind {
            b"IHDR" if payload.len() >= 13 => {
                let width = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let height = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                dimensions = Some((width, height));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "ImageWidth",
                    Value::Int(width as i64),
                ));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "ImageHeight",
                    Value::Int(height as i64),
                ));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "BitDepth",
                    Value::Int(payload[8] as i64),
                ));
                let color = match payload[9] {
                    0 => "Grayscale",
                    2 => "RGB",
                    3 => "Palette",
                    4 => "Grayscale with Alpha",
                    6 => "RGB with Alpha",
                    _ => "Unknown",
                };
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "ColorType",
                    Value::Str(color.to_string()),
                ));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "Compression",
                    Value::Str("Deflate/Inflate".to_string()),
                ));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "Filter",
                    Value::Str("Adaptive".to_string()),
                ));
                out.push(Tag::new(
                    "PNG",
                    "PNG",
                    "Interlace",
                    Value::Str(
                        if payload[12] == 0 {
                            "Noninterlaced"
                        } else {
                            "Adam7 Interlace"
                        }
                        .to_string(),
                    ),
                ));
            }
            b"eXIf" => {
                let little_endian = payload.starts_with(b"II");
                crate::read::push_byte_order(out, little_endian);
                let mut exif_tags = Vec::new();
                exif::parse(payload, little_endian, &mut exif_tags, warnings, false);
                out.extend(exif_tags);
            }
            b"iTXt" => {
                if let Some((keyword, text)) = itxt(payload)
                    && keyword == "XML:com.adobe.xmp"
                {
                    crate::xmp::parse(text.as_bytes(), out, warnings);
                }
            }
            b"tEXt" => {
                if let Some((keyword, text)) = text_chunk(payload)
                    && keyword == "XML:com.adobe.xmp"
                {
                    crate::xmp::parse(text.as_bytes(), out, warnings);
                }
            }
            b"sRGB" if !payload.is_empty() => {
                let label = match payload[0] {
                    0 => "Perceptual",
                    1 => "Relative Colorimetric",
                    2 => "Saturation",
                    3 => "Absolute Colorimetric",
                    _ => "Unknown",
                };
                out.push(
                    Tag::new("PNG", "PNG", "SRGBRendering", Value::Int(payload[0] as i64))
                        .with_print(label),
                );
            }
            b"tIME" if payload.len() >= 7 => {
                let stamp = format!(
                    "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
                    u16::from_be_bytes([payload[0], payload[1]]),
                    payload[2],
                    payload[3],
                    payload[4],
                    payload[5],
                    payload[6]
                );
                out.push(Tag::new("PNG", "PNG", "ModifyDate", Value::Str(stamp)));
            }
            _ => {}
        }
        pos = end + 4;
    }
    dimensions
}

/// Extract the TIFF stream from the first `eXIf` chunk.
pub fn extract_exif(data: &[u8]) -> Option<&[u8]> {
    let mut pos = 8usize;
    while pos + 12 <= data.len() {
        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind = &data[pos + 4..pos + 8];
        let start = pos + 8;
        let end = start.checked_add(length)?;
        if end + 4 > data.len() {
            return None;
        }
        if kind == b"eXIf" {
            return Some(&data[start..end]);
        }
        pos = end + 4;
    }
    None
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    let mut crc_data = Vec::with_capacity(4 + payload.len());
    crc_data.extend_from_slice(kind);
    crc_data.extend_from_slice(payload);
    out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

/// Replace an existing `eXIf` chunk or insert one before `IDAT`.
pub fn replace_exif(data: &[u8], tiff: &[u8]) -> Result<Vec<u8>, String> {
    if tiff.is_empty() {
        return Err("empty eXIf payload".to_string());
    }
    let mut out = Vec::with_capacity(data.len() + tiff.len() + 12);
    out.extend_from_slice(&data[..8.min(data.len())]);
    let mut pos = 8usize;
    let mut replaced = false;
    while pos + 12 <= data.len() {
        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let end = pos + 8 + length;
        if end + 4 > data.len() {
            return Err("PNG: truncated chunk".to_string());
        }
        let kind = &data[pos + 4..pos + 8];
        if kind == b"eXIf" {
            push_chunk(&mut out, b"eXIf", tiff);
            replaced = true;
        } else if kind == b"IDAT" && !replaced {
            push_chunk(&mut out, b"eXIf", tiff);
            replaced = true;
            out.extend_from_slice(&data[pos..end + 4]);
        } else {
            out.extend_from_slice(&data[pos..end + 4]);
        }
        pos = end + 4;
    }
    if !replaced {
        return Err("PNG: no IDAT chunk for eXIf insertion".to_string());
    }
    Ok(out)
}

fn type_size(typ: u16) -> Option<usize> {
    match typ {
        1 | 2 | 6 | 7 => Some(1),
        3 | 8 => Some(2),
        4 | 9 | 11 | 13 => Some(4),
        5 | 10 | 12 => Some(8),
        _ => None,
    }
}

fn read_u16(data: &[u8], at: usize, le: bool) -> Option<u16> {
    let raw = data.get(at..at + 2)?;
    Some(if le {
        u16::from_le_bytes([raw[0], raw[1]])
    } else {
        u16::from_be_bytes([raw[0], raw[1]])
    })
}

fn read_u32(data: &[u8], at: usize, le: bool) -> Option<u32> {
    let raw = data.get(at..at + 4)?;
    Some(if le {
        u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])
    } else {
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]])
    })
}

fn write_u16(data: &mut [u8], at: usize, value: u16, le: bool) {
    let raw = if le {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    data[at..at + 2].copy_from_slice(&raw);
}

fn write_u32(data: &mut [u8], at: usize, value: u32, le: bool) {
    let raw = if le {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    data[at..at + 4].copy_from_slice(&raw);
}

struct TiffEntries {
    le: bool,
    entries: Vec<(u16, u16, u32, Vec<u8>)>,
    next: u32,
}

fn parse_ifd0(data: &[u8]) -> Result<TiffEntries, String> {
    if data.len() < 8 {
        return Err("invalid TIFF header".to_string());
    }
    let le = match &data[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("invalid TIFF byte order".to_string()),
    };
    let offset = read_u32(data, 4, le).ok_or_else(|| "invalid TIFF offset".to_string())? as usize;
    let count = read_u16(data, offset, le).ok_or_else(|| "invalid IFD0".to_string())? as usize;
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let at = offset + 2 + index * 12;
        let tag = read_u16(data, at, le).ok_or_else(|| "IFD0 entry outside file".to_string())?;
        let typ =
            read_u16(data, at + 2, le).ok_or_else(|| "IFD0 entry outside file".to_string())?;
        let value_count =
            read_u32(data, at + 4, le).ok_or_else(|| "IFD0 entry outside file".to_string())?;
        let size = type_size(typ).ok_or_else(|| "unknown TIFF type".to_string())?;
        let total = size
            .checked_mul(value_count as usize)
            .ok_or_else(|| "TIFF value too large".to_string())?;
        let bytes = if total <= 4 {
            data.get(at + 8..at + 8 + total)
                .ok_or_else(|| "IFD0 value outside file".to_string())?
                .to_vec()
        } else {
            let value_offset = read_u32(data, at + 8, le)
                .ok_or_else(|| "IFD0 value outside file".to_string())?
                as usize;
            data.get(value_offset..value_offset + total)
                .ok_or_else(|| "IFD0 value outside file".to_string())?
                .to_vec()
        };
        entries.push((tag, typ, value_count, bytes));
    }
    let next = read_u32(data, offset + 2 + count * 12, le)
        .ok_or_else(|| "IFD0 next pointer outside file".to_string())?;
    Ok(TiffEntries { le, entries, next })
}

fn date_bytes(value: &str) -> Vec<u8> {
    let mut bytes = value.as_bytes().to_vec();
    bytes.resize(20, 0);
    bytes[19] = 0;
    bytes
}

/// Whether the TIFF stream has an ExifIFD pointer in IFD0.
pub fn has_exif_ifd(data: &[u8]) -> bool {
    parse_ifd0(data)
        .map(|entry| entry.entries.iter().any(|entry| entry.0 == 0x8769))
        .unwrap_or(false)
}

/// Build a TIFF stream with dates. With `base`, preserve existing IFD0 entries
/// (used for PNG eXIf without an ExifIFD pointer).
pub fn build_exif(base: Option<&[u8]>, updates: &[(String, String)]) -> Result<Vec<u8>, String> {
    let parsed = base.map(parse_ifd0).transpose()?;
    let le = parsed.as_ref().map(|entry| entry.le).unwrap_or(true);
    let old_len = base.map(|data| data.len()).unwrap_or(0);
    let ifd_offset = if base.is_some() {
        old_len + (old_len & 1)
    } else {
        8
    };
    let existing = parsed
        .as_ref()
        .map(|entry| entry.entries.clone())
        .unwrap_or_default();
    let next = parsed.as_ref().map(|entry| entry.next).unwrap_or(0);
    let modifier = updates
        .iter()
        .find(|(name, _)| name == "ModifyDate")
        .map(|(_, value)| date_bytes(value));
    let exif_dates: Vec<(u16, Vec<u8>)> = updates
        .iter()
        .filter_map(|(name, value)| match name.as_str() {
            "DateTimeOriginal" => Some((0x9003, date_bytes(value))),
            "CreateDate" => Some((0x9004, date_bytes(value))),
            _ => None,
        })
        .collect();
    if modifier.is_none() && exif_dates.is_empty() {
        return Err("no dates to write".to_string());
    }
    let has_pointer = existing.iter().any(|entry| entry.0 == 0x8769);
    if has_pointer && !exif_dates.is_empty() {
        return Err("existing ExifIFD must be updated in place".to_string());
    }
    let mut ifd0: Vec<(u16, u16, u32, Vec<u8>)> = existing
        .iter()
        .filter(|entry| entry.0 != 0x8769 && entry.0 != 0x9003 && entry.0 != 0x9004)
        .cloned()
        .collect();
    let old_modify = ifd0.iter().position(|entry| entry.0 == 0x0132);
    if let Some(bytes) = modifier {
        if let Some(index) = old_modify {
            ifd0[index] = (0x0132, 2, 20, bytes);
        } else {
            ifd0.push((0x0132, 2, 20, bytes));
        }
    }
    if !exif_dates.is_empty() {
        let pointer_index = ifd0.iter().position(|entry| entry.0 == 0x8769);
        if pointer_index.is_none() {
            ifd0.push((0x8769, 4, 1, vec![0, 0, 0, 0]));
        }
    }
    if has_pointer && exif_dates.is_empty() {
        // Keep an existing pointer untouched when only ModifyDate changes.
        ifd0.extend(existing.iter().filter(|entry| entry.0 == 0x8769).cloned());
    }
    ifd0.sort_by_key(|entry| entry.0);
    let count = ifd0.len();
    let table_end = ifd_offset + 2 + count * 12 + 4;
    let mut values = Vec::new();
    let mut encoded = Vec::with_capacity(count);
    for (tag, typ, value_count, bytes) in ifd0 {
        if bytes.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..bytes.len()].copy_from_slice(&bytes);
            encoded.push((
                tag,
                typ,
                value_count,
                u32::from_le_bytes(inline) as u64,
                inline,
            ));
        } else {
            let at = table_end + values.len();
            values.extend_from_slice(&bytes);
            if values.len() & 1 != 0 {
                values.push(0);
            }
            encoded.push((tag, typ, value_count, at as u64, [0u8; 4]));
        }
    }
    let exif_offset = {
        let mut at = table_end + values.len();
        if at & 1 != 0 {
            at += 1;
        }
        at
    };
    let mut exif_count = 0usize;
    if !exif_dates.is_empty() {
        exif_count = exif_dates.len();
    }
    let exif_table_end = if exif_count == 0 {
        exif_offset
    } else {
        exif_offset + 2 + exif_count * 12 + 4
    };
    let mut exif_values = Vec::new();
    let mut exif_entries = Vec::new();
    for (tag, bytes) in &exif_dates {
        let at = exif_table_end + exif_values.len();
        exif_values.extend_from_slice(bytes);
        if exif_values.len() & 1 != 0 {
            exif_values.push(0);
        }
        exif_entries.push((*tag, 2u16, 20u32, at as u32));
    }
    let total = exif_table_end + exif_values.len();
    let mut out = vec![0u8; total.max(ifd_offset + 2 + count * 12 + 4 + values.len())];
    if let Some(base) = base {
        out[..base.len()].copy_from_slice(base);
    } else {
        out[..4].copy_from_slice(b"II*\0");
        write_u32(&mut out, 4, 8, le);
    }
    write_u32(&mut out, 4, ifd_offset as u32, le);
    write_u16(&mut out, ifd_offset, count as u16, le);
    for (index, (tag, typ, value_count, at, inline)) in encoded.iter().enumerate() {
        let entry_at = ifd_offset + 2 + index * 12;
        write_u16(&mut out, entry_at, *tag, le);
        write_u16(&mut out, entry_at + 2, *typ, le);
        write_u32(&mut out, entry_at + 4, *value_count, le);
        if *tag == 0x8769 && exif_count > 0 {
            write_u32(&mut out, entry_at + 8, exif_offset as u32, le);
        } else if *at == 0 {
            out[entry_at + 8..entry_at + 12].copy_from_slice(inline);
        } else {
            write_u32(&mut out, entry_at + 8, *at as u32, le);
        }
    }
    write_u32(&mut out, ifd_offset + 2 + count * 12, next, le);
    let values_at = ifd_offset + 2 + count * 12 + 4;
    out[values_at..values_at + values.len()].copy_from_slice(&values);
    if exif_count > 0 {
        write_u16(&mut out, exif_offset, exif_count as u16, le);
        for (index, (tag, typ, value_count, at)) in exif_entries.iter().enumerate() {
            let entry_at = exif_offset + 2 + index * 12;
            write_u16(&mut out, entry_at, *tag, le);
            write_u16(&mut out, entry_at + 2, *typ, le);
            write_u32(&mut out, entry_at + 4, *value_count, le);
            write_u32(&mut out, entry_at + 8, *at, le);
        }
        write_u32(&mut out, exif_offset + 2 + exif_count * 12, 0, le);
        out[exif_table_end..exif_table_end + exif_values.len()].copy_from_slice(&exif_values);
    }
    Ok(out)
}
