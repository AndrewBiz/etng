//! EXIF extraction on top of `kamadak-exif`, mapped to ExifTool names.

use etng_tags::tables;
use exif::{Context, In, Value as ExifValue};

use crate::makernotes;
use crate::model::{Tag, Value};
use crate::tiff::Tiff;

/// Parse EXIF from a JPEG/TIFF container (or a raw TIFF payload).
pub fn parse(
    data: &[u8],
    little_endian: bool,
    out: &mut Vec<Tag>,
    warnings: &mut Vec<String>,
    numeric: bool,
) {
    let mut cursor = std::io::Cursor::new(data);
    let exif = match exif::Reader::new()
        .continue_on_error(true)
        .read_from_container(&mut cursor)
    {
        Ok(exif) => exif,
        Err(err) => match err.distill_partial_result(|errors| {
            warnings.push(format!("EXIF: partial result ({} errors)", errors.len()));
        }) {
            Ok(exif) => exif,
            Err(err) => {
                warnings.push(format!("EXIF: {err}"));
                return;
            }
        },
    };

    let mut make: Option<String> = None;
    let mut maker_note: Option<Vec<u8>> = None;
    let maker_offset = find_maker_note_offset(data);

    for field in exif.fields() {
        let g1 = match field.tag.context() {
            Context::Tiff => {
                if field.ifd_num == In::THUMBNAIL {
                    "IFD1"
                } else {
                    "IFD0"
                }
            }
            Context::Exif => "ExifIFD",
            Context::Gps => "GPS",
            Context::Interop => "InteropIFD",
            _ => continue,
        };
        let table = if g1 == "GPS" {
            "GPS::Main"
        } else {
            "Exif::Main"
        };
        let id = field.tag.number().to_string();
        let Some(def) = tables().find_table_id(table, &id) else {
            continue;
        };

        if def.name == "Make"
            && let ExifValue::Ascii(parts) = &field.value
            && let Some(first) = parts.first()
        {
            make = Some(
                String::from_utf8_lossy(first)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string(),
            );
        }
        if def.name.starts_with("MakerNote") {
            match &field.value {
                ExifValue::Undefined(bytes, _)
                    if def.name == "MakerNote" || def.name == "MakerNoteApple" =>
                {
                    maker_note = Some(bytes.clone());
                }
                ExifValue::Undefined(..) => {
                    warnings.push(format!("MakerNotes: raw {} block skipped", def.name));
                }
                _ => {}
            }
            continue;
        }

        let Some(value) = to_value(&field.value, def) else {
            continue;
        };
        let value = if matches!(
            def.name,
            "StripOffsets" | "StripByteCounts" | "ThumbnailOffset" | "ThumbnailLength"
        ) && let Value::Str(text) = &value
            && text.contains(' ')
        {
            Value::Bytes(text.clone().into_bytes())
        } else {
            value
        };

        let value = if g1 == "GPS" && matches!(def.name, "GPSLatitude" | "GPSLongitude") {
            match gps_decimal(&field.value) {
                Some(decimal) => Value::Float(decimal),
                None => value,
            }
        } else {
            value
        };

        let mut tag = Tag::new("EXIF", g1, def.name, value);
        if !numeric {
            if let Some(print) = special_print(def.name, &tag.value) {
                tag = tag.with_print(print);
            } else if let Some(print) = def.print_value(&tag.value.display()) {
                tag = tag.with_print(print);
            }
        }
        out.push(tag);
    }

    parse_subifds(data, out, numeric);
    if make
        .as_deref()
        .is_some_and(|text| text.to_ascii_uppercase().contains("SONY"))
    {
        sony_preview(data, out);
    }

    gps_prints(out);
    if let Some(note) = maker_note {
        let is_canon = make
            .as_deref()
            .is_some_and(|text| text.to_ascii_uppercase().contains("CANON"));
        if is_canon
            && let Some(offset) = maker_offset
            && makernotes::parse_canon_at(data, offset, out, warnings, numeric)
        {
            return;
        }
        makernotes::parse(
            make.as_deref(),
            &note,
            little_endian,
            out,
            warnings,
            numeric,
        );
    }
}

/// Locate the MakerNote value offset relative to the TIFF header.
fn find_maker_note_offset(data: &[u8]) -> Option<u32> {
    let tiff = crate::tiff::Tiff::new(data)?;
    let mut queue = vec![tiff.first_ifd_offset()?];
    let mut seen = std::collections::HashSet::new();
    while let Some(offset) = queue.pop() {
        if !seen.insert(offset) {
            continue;
        }
        let Some(ifd) = tiff.read_ifd(offset) else {
            continue;
        };
        for entry in &ifd.entries {
            if entry.tag == 0x927C && entry.typ == 7 && entry.count > 4 {
                return Some(entry.value_or_offset);
            }
            if matches!(entry.tag, 0x8769 | 0x8825 | 0xA005 | 0x014A) {
                queue.push(entry.value_or_offset);
            }
        }
        if ifd.next != 0 {
            queue.push(ifd.next);
        }
    }
    None
}

fn gps_prints(tags: &mut [Tag]) {
    for tag in tags.iter_mut() {
        if !matches!(tag.name, "GPSLatitude" | "GPSLongitude") {
            continue;
        }
        if let Some(decimal) = tag.value.as_f64()
            && let Some(print) = format_coordinate(decimal, None)
        {
            tag.print = Some(print);
        }
    }
}

pub(crate) fn format_coordinate(decimal: f64, reference: Option<char>) -> Option<String> {
    let value = decimal.abs();
    let degrees = value.trunc();
    let minutes_full = (value - degrees) * 60.0;
    let minutes = minutes_full.trunc();
    let seconds = (minutes_full - minutes) * 60.0;
    let suffix = reference.map(|ch| format!(" {ch}")).unwrap_or_default();
    Some(format!(
        "{degrees:.0} deg {minutes:.0}' {seconds:.2}\"{suffix}"
    ))
}

pub(crate) fn special_print(name: &str, value: &Value) -> Option<String> {
    match name {
        "FocalLength" => value.as_f64().map(|mm| format!("{mm:.1} mm")),
        "FocalPlaneXResolution" | "FocalPlaneYResolution" => {
            value.as_f64().map(crate::formatting::format_float)
        }
        "FocalLengthIn35mmFormat" => value
            .as_f64()
            .map(|mm| format!("{} mm", crate::formatting::format_float(mm))),
        "GPSAltitude" | "GPSHPositioningError" => value
            .as_f64()
            .map(|metres| format!("{} m", crate::formatting::format_float(metres))),
        "GPSLatitude" | "GPSLongitude" => value
            .as_f64()
            .and_then(|decimal| format_coordinate(decimal, None)),
        "ApertureValue" | "MaxApertureValue" => value.as_f64().map(|apex| {
            let fnumber = 2f64.powf(apex / 2.0);
            format!("{fnumber:.1}")
        }),
        "FNumber" => value.as_f64().map(|fnumber| format!("{fnumber:.1}")),
        "ExposureTime" => value.as_f64().map(|seconds| {
            if seconds > 0.0 && seconds < 1.0 {
                format!("1/{}", (1.0 / seconds).round() as i64)
            } else {
                crate::formatting::format_float(seconds)
            }
        }),
        "GPSTimeStamp" => gps_time_print(value),
        "BrightnessValue" | "GPSDestBearing" | "GPSImgDirection" => {
            value.as_f64().map(crate::formatting::format_float)
        }
        "ShutterSpeedValue" => value.as_f64().map(|apex| {
            let seconds = 2f64.powf(-apex);
            if seconds < 1.0 {
                format!("1/{}", (1.0 / seconds).round() as i64)
            } else {
                crate::formatting::format_float(seconds)
            }
        }),
        "LensInfo" => lens_info_print(value),
        name if name.ends_with("Version") => match value {
            Value::Int(raw) => {
                let bytes = (*raw as u64).to_be_bytes();
                Some(
                    bytes[4..]
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join("."),
                )
            }
            _ => None,
        },
        _ => None,
    }
}

fn gps_time_print(value: &Value) -> Option<String> {
    let Value::Str(text) = value else {
        return value.as_f64().map(|seconds| {
            let total = seconds.round() as u64;
            format!(
                "{:02}:{:02}:{:02}",
                total / 3600,
                (total / 60) % 60,
                total % 60
            )
        });
    };
    let parts: Vec<f64> = text
        .split_whitespace()
        .filter_map(|token| match token.split_once('/') {
            Some((num, den)) => {
                let num: f64 = num.parse().ok()?;
                let den: f64 = den.parse().ok()?;
                if den == 0.0 { None } else { Some(num / den) }
            }
            None => token.parse().ok(),
        })
        .collect();
    if parts.len() != 3 {
        return None;
    }
    Some(format!(
        "{:02}:{:02}:{:02}",
        parts[0].round() as u64,
        parts[1].round() as u64,
        parts[2].round() as u64
    ))
}

fn lens_info_print(value: &Value) -> Option<String> {
    let Value::Str(text) = value else {
        return None;
    };
    let numbers: Vec<f64> = text
        .split_whitespace()
        .filter_map(|token| match token.split_once('/') {
            Some((num, den)) => {
                let num: f64 = num.parse().ok()?;
                let den: f64 = den.parse().ok()?;
                if den == 0.0 { None } else { Some(num / den) }
            }
            None => token.parse().ok(),
        })
        .collect();
    if numbers.len() != 4 {
        return None;
    }
    Some(format!(
        "{}-{}mm f/{}-{}",
        crate::formatting::format_float(numbers[0]),
        crate::formatting::format_float(numbers[1]),
        crate::formatting::format_float(numbers[2]),
        crate::formatting::format_float(numbers[3])
    ))
}

/// Format a coordinate with its hemisphere reference (used by composites).
pub fn coordinate_with_ref(decimal: f64, reference: char) -> String {
    format_coordinate(decimal, Some(reference)).unwrap_or_default()
}

fn gps_decimal(value: &ExifValue) -> Option<f64> {
    let items: Vec<(f64, f64)> = match value {
        ExifValue::Rational(items) if items.len() == 3 => items
            .iter()
            .map(|r| (r.num as f64, r.denom as f64))
            .collect(),
        ExifValue::SRational(items) if items.len() == 3 => items
            .iter()
            .map(|r| (r.num as f64, r.denom as f64))
            .collect(),
        _ => return None,
    };
    if items.iter().any(|(_, denom)| *denom == 0.0) {
        return None;
    }
    let degrees = items[0].0 / items[0].1;
    let minutes = items[1].0 / items[1].1;
    let seconds = items[2].0 / items[2].1;
    Some(degrees + minutes / 60.0 + seconds / 3600.0)
}

fn parse_subifds(data: &[u8], out: &mut Vec<Tag>, numeric: bool) {
    let Some(tiff) = Tiff::new(data) else {
        return;
    };
    let Some(ifd_offset) = tiff.first_ifd_offset() else {
        return;
    };
    let Some(ifd) = tiff.read_ifd(ifd_offset) else {
        return;
    };
    let Some(entry) = ifd.entries.iter().find(|entry| entry.tag == 0x014a) else {
        return;
    };
    let Some(bytes) = tiff.entry_bytes(entry) else {
        return;
    };
    let Some(offset) = bytes.as_chunks::<4>().0.first().map(|chunk| {
        if tiff.le {
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        } else {
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        }
    }) else {
        return;
    };
    let Some(sub) = tiff.read_ifd(offset) else {
        return;
    };
    for entry in &sub.entries {
        let id = entry.tag.to_string();
        let Some(def) = tables().find_table_id("Exif::Main", &id) else {
            continue;
        };
        let Some(value) = makernotes::entry_value(&tiff, entry) else {
            continue;
        };
        if matches!(value, Value::Bytes(_)) {
            continue;
        }
        let mut tag = Tag::new("EXIF", "SubIFD", def.name, value);
        if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
            tag = tag.with_print(print);
        }
        out.push(tag);
    }
}

fn sony_preview(data: &[u8], out: &mut Vec<Tag>) {
    let Some(tiff) = Tiff::new(data) else {
        return;
    };
    let Some(ifd) = tiff
        .first_ifd_offset()
        .and_then(|offset| tiff.read_ifd(offset))
    else {
        return;
    };
    let read_u32 = |tag: u16| {
        ifd.entries
            .iter()
            .find(|entry| entry.tag == tag)
            .and_then(|entry| tiff.entry_bytes(entry))
            .and_then(|bytes| bytes.get(0..4))
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let (Some(start), Some(length)) = (read_u32(0x0201), read_u32(0x0202)) else {
        return;
    };
    let start = start as usize;
    let length = length as usize;
    let Some(end) = start.checked_add(length) else {
        return;
    };
    let Some(bytes) = data.get(start..end) else {
        return;
    };
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return;
    }
    out.push(Tag::new(
        "EXIF",
        "IFD0",
        "PreviewImageStart",
        Value::Int(start as i64),
    ));
    out.push(Tag::new(
        "EXIF",
        "IFD0",
        "PreviewImageLength",
        Value::Int(length as i64),
    ));
    out.push(Tag::new(
        "EXIF",
        "IFD0",
        "PreviewImage",
        Value::Bytes(bytes.to_vec()),
    ));
}

fn to_value(value: &ExifValue, def: &etng_tags::TagDef) -> Option<Value> {
    Some(match value {
        ExifValue::Ascii(parts) => Value::Str(
            parts
                .first()
                .map(|bytes| bytes.as_slice())
                .map(|bytes| {
                    String::from_utf8_lossy(bytes)
                        .trim_end_matches('\0')
                        .to_string()
                })
                .unwrap_or_default(),
        ),
        ExifValue::Byte(items) => {
            if def.name == "GPSVersionID" {
                Value::Str(
                    items
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join("."),
                )
            } else {
                ints(items.iter().map(|v| *v as i64))
            }
        }
        ExifValue::SByte(items) => ints(items.iter().map(|v| *v as i64)),
        ExifValue::Short(items) => ints(items.iter().map(|v| *v as i64)),
        ExifValue::SShort(items) => ints(items.iter().map(|v| *v as i64)),
        ExifValue::Long(items) => ints(items.iter().map(|v| *v as i64)),
        ExifValue::SLong(items) => ints(items.iter().map(|v| *v as i64)),
        ExifValue::Rational(items) => {
            rationals(items.iter().map(|r| (r.num as i64, r.denom as i64)))
        }
        ExifValue::SRational(items) => {
            rationals(items.iter().map(|r| (r.num as i64, r.denom as i64)))
        }
        ExifValue::Float(items) => floats(items.iter().map(|v| *v as f64)),
        ExifValue::Double(items) => floats(items.iter().copied()),
        ExifValue::Undefined(bytes, _) => {
            if def.name == "UserComment" {
                let body = bytes.get(8..).unwrap_or_default();
                return Some(Value::Str(
                    String::from_utf8_lossy(body)
                        .trim_matches('\0')
                        .trim()
                        .to_string(),
                ));
            }
            if bytes.iter().all(|byte| (0x20..0x7F).contains(byte)) {
                Value::Str(
                    String::from_utf8_lossy(bytes)
                        .trim_end_matches('\0')
                        .to_string(),
                )
            } else if def.has_values() {
                let labels: Vec<&str> = bytes
                    .iter()
                    .map(|byte| def.print_value(&byte.to_string()).unwrap_or("?"))
                    .collect();
                Value::Str(labels.join(", "))
            } else {
                Value::Bytes(bytes.clone())
            }
        }
        _ => return None,
    })
}

fn ints(values: impl Iterator<Item = i64>) -> Value {
    let items: Vec<i64> = values.collect();
    match items.as_slice() {
        [] => Value::Str(String::new()),
        [single] => Value::Int(*single),
        many => Value::Str(
            many.iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
}

fn rationals(values: impl Iterator<Item = (i64, i64)>) -> Value {
    let items: Vec<(i64, i64)> = values.collect();
    match items.as_slice() {
        [] => Value::Str(String::new()),
        [(num, den)] => Value::Rational {
            num: *num,
            den: *den,
        },
        many => Value::Str(
            many.iter()
                .map(|(n, d)| format!("{n}/{d}"))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
}

fn floats(values: impl Iterator<Item = f64>) -> Value {
    let items: Vec<f64> = values.collect();
    match items.as_slice() {
        [] => Value::Str(String::new()),
        [single] => Value::Float(*single),
        many => Value::Str(
            many.iter()
                .map(|v| crate::formatting::format_float(*v))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
}

#[cfg(test)]
mod subifd_tests {
    use super::*;

    fn tiff_with_subifd() -> Vec<u8> {
        let mut data = vec![0u8; 128];
        data[0..4].copy_from_slice(b"II*\0");
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        data[8..10].copy_from_slice(&3u16.to_le_bytes());
        let entry = |data: &mut [u8], at: usize, tag: u16, typ: u16, count: u32, value: u32| {
            data[at..at + 2].copy_from_slice(&tag.to_le_bytes());
            data[at + 2..at + 4].copy_from_slice(&typ.to_le_bytes());
            data[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
            data[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
        };
        entry(&mut data, 10, 0x014a, 4, 1, 64);
        entry(&mut data, 22, 0x0201, 4, 1, 128);
        entry(&mut data, 34, 0x0202, 4, 1, 4);
        data[46..50].copy_from_slice(&0u32.to_le_bytes());

        data[64..66].copy_from_slice(&2u16.to_le_bytes());
        entry(&mut data, 66, 0x0100, 4, 1, 100);
        entry(&mut data, 78, 0x0101, 4, 1, 50);
        data[90..94].copy_from_slice(&0u32.to_le_bytes());

        data.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xD9]);
        data
    }

    #[test]
    fn reads_subifd_dimensions() {
        let data = tiff_with_subifd();
        let mut tags = Vec::new();
        parse_subifds(&data, &mut tags, false);
        let width = tags.iter().find(|tag| tag.name == "ImageWidth").unwrap();
        assert_eq!(width.value.display(), "100");
        assert_eq!(width.group1, "SubIFD");
    }

    #[test]
    fn extracts_sony_preview_bytes() {
        let data = tiff_with_subifd();
        let mut tags = Vec::new();
        sony_preview(&data, &mut tags);
        let preview = tags
            .iter()
            .find(|tag| tag.name == "PreviewImage")
            .expect("preview");
        assert!(matches!(
            &preview.value,
            Value::Bytes(bytes) if bytes == &[0xFF, 0xD8, 0xFF, 0xD9]
        ));
        assert!(tags.iter().any(|tag| tag.name == "PreviewImageStart"));
        assert!(tags.iter().any(|tag| tag.name == "PreviewImageLength"));
    }
}
