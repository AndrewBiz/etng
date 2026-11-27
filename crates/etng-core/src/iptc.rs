//! IPTC IIM parsing from the Photoshop APP13 resource block.

use etng_tags::tables;

use crate::model::{Tag, Value};

const SIGNATURE: &[u8] = b"8BIM";
const IPTC_RESOURCE: u16 = 0x0404;

/// Parse an IPTC IIM byte stream (already extracted from the Photoshop block).
pub fn parse_iim(data: &[u8], out: &mut Vec<Tag>) {
    let mut pos = 0usize;
    while pos + 5 <= data.len() {
        if data[pos] != 0x1C {
            pos += 1;
            continue;
        }
        let record = data[pos + 1];
        let dataset = data[pos + 2];
        let mut size = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;
        let mut header = 5usize;
        if size & 0x8000 != 0 {
            let count = size & 0x7FFF;
            if count == 0 || count > 4 || pos + 5 + count > data.len() {
                break;
            }
            size = match count {
                1 => data[pos + 5] as usize,
                2 => u16::from_be_bytes([data[pos + 5], data[pos + 6]]) as usize,
                3 => u32::from_be_bytes([0, data[pos + 5], data[pos + 6], data[pos + 7]]) as usize,
                _ => {
                    u32::from_be_bytes([data[pos + 5], data[pos + 6], data[pos + 7], data[pos + 8]])
                        as usize
                }
            };
            header = 5 + count;
        }
        let start = pos + header;
        let end = start.saturating_add(size);
        if end > data.len() {
            break;
        }
        if let Some(table) = table_for_record(record) {
            let id = dataset.to_string();
            if let Some(def) = tables().find_table_id(table, &id) {
                let raw = &data[start..end];
                let value = match def.name {
                    "DateCreated" | "DigitalCreationDate" => {
                        Value::Str(iptc_date(&String::from_utf8_lossy(raw)))
                    }
                    "TimeCreated" | "DigitalCreationTime" => {
                        Value::Str(iptc_time(&String::from_utf8_lossy(raw)))
                    }
                    _ => match def.typ {
                        "int16u" => Value::Int(u16::from_be_bytes([raw[0], raw[1]]) as i64),
                        "int16s" => Value::Int(i16::from_be_bytes([raw[0], raw[1]]) as i64),
                        "int32u" if raw.len() >= 4 => {
                            Value::Int(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as i64)
                        }
                        _ => iim_value(raw),
                    },
                };
                if let Some(existing) = out
                    .iter_mut()
                    .find(|tag| tag.name == def.name && tag.group1 == def.g1)
                {
                    if let (Value::Str(current), Value::Str(extra)) = (&mut existing.value, &value)
                    {
                        current.push_str(", ");
                        current.push_str(extra);
                    }
                } else {
                    let mut tag = Tag::new("IPTC", def.g1, def.name, value);
                    if let Some(print) = def.print_value(&tag.value.display()) {
                        tag = tag.with_print(print);
                    }
                    out.push(tag);
                }
            }
        }
        pos = end;
    }
}

fn iptc_date(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() == 8 && raw.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{}:{}:{}", &raw[0..4], &raw[4..6], &raw[6..8])
    } else {
        raw.to_string()
    }
}

fn iptc_time(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 6
        && raw.bytes().take(6).all(|byte| byte.is_ascii_digit())
        && raw.as_bytes()[2] != b':'
    {
        let mut out = format!("{}:{}:{}", &raw[0..2], &raw[2..4], &raw[4..6]);
        let rest = &raw[6..];
        if rest.len() == 5 && (rest.starts_with('+') || rest.starts_with('-')) {
            out.push_str(&rest[0..3]);
            out.push(':');
            out.push_str(&rest[3..5]);
        } else if rest == "Z" {
            out.push('Z');
        }
        out
    } else {
        raw.to_string()
    }
}

fn table_for_record(record: u8) -> Option<&'static str> {
    Some(match record {
        1 => "IPTC::EnvelopeRecord",
        2 => "IPTC::ApplicationRecord",
        3 => "IPTC::NewsPhoto",
        7 => "IPTC::PreObjectData",
        8 => "IPTC::ObjectData",
        9 => "IPTC::PostObjectData",
        _ => return None,
    })
}

fn iim_value(raw: &[u8]) -> Value {
    let printable = raw
        .iter()
        .all(|&b| b == b'\n' || b == b'\r' || b == b'\t' || (0x20..0x7F).contains(&b));
    if printable {
        Value::Str(String::from_utf8_lossy(raw).trim().to_string())
    } else {
        Value::Bytes(raw.to_vec())
    }
}

/// Walk the Photoshop 8BIM resources and parse the IPTC resource.
pub fn parse_photoshop(block: &[u8], out: &mut Vec<Tag>, warnings: &mut Vec<String>) {
    let mut pos = 0usize;
    let mut found = false;
    while pos + 12 <= block.len() {
        if &block[pos..pos + 4] != SIGNATURE {
            pos += 1;
            continue;
        }
        let resource_id = u16::from_be_bytes([block[pos + 4], block[pos + 5]]);
        let name_len = block[pos + 6] as usize;
        let name_total = if name_len.is_multiple_of(2) {
            name_len + 2
        } else {
            name_len + 1
        };
        let size_pos = pos + 6 + name_total;
        if size_pos + 4 > block.len() {
            break;
        }
        let size = u32::from_be_bytes([
            block[size_pos],
            block[size_pos + 1],
            block[size_pos + 2],
            block[size_pos + 3],
        ]) as usize;
        let data_start = size_pos + 4;
        let data_end = data_start.saturating_add(size);
        if data_end > block.len() {
            warnings.push("IPTC resource truncated".to_string());
            break;
        }
        if resource_id == IPTC_RESOURCE {
            parse_iim(&block[data_start..data_end], out);
            found = true;
        }
        pos = data_end + (size & 1);
    }
    if !found && !block.is_empty() {
        // Some writers put bare IIM data into APP13.
        parse_iim(block, out);
    }
}

/// Walk the Photoshop 8BIM resources and return the raw IPTC IIM payload.
pub fn extract_iim(block: &[u8]) -> Option<&[u8]> {
    let mut pos = 0usize;
    while pos + 12 <= block.len() {
        if &block[pos..pos + 4] != SIGNATURE {
            pos += 1;
            continue;
        }
        let resource_id = u16::from_be_bytes([block[pos + 4], block[pos + 5]]);
        let name_len = block[pos + 6] as usize;
        let name_total = if name_len.is_multiple_of(2) {
            name_len + 2
        } else {
            name_len + 1
        };
        let size_pos = pos + 6 + name_total;
        if size_pos + 4 > block.len() {
            return None;
        }
        let size = u32::from_be_bytes([
            block[size_pos],
            block[size_pos + 1],
            block[size_pos + 2],
            block[size_pos + 3],
        ]) as usize;
        let data_start = size_pos + 4;
        let data_end = data_start.saturating_add(size);
        if data_end > block.len() {
            return None;
        }
        if resource_id == IPTC_RESOURCE {
            return Some(&block[data_start..data_end]);
        }
        pos = data_end + (size & 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iim(record: u8, dataset: u8, value: &[u8]) -> Vec<u8> {
        let mut out = vec![0x1C, record, dataset];
        out.extend_from_slice(&(value.len() as u16).to_be_bytes());
        out.extend_from_slice(value);
        out
    }

    #[test]
    fn parses_keywords_and_city() {
        let mut data = iim(2, 25, b"night");
        data.extend(iim(2, 25, b"street"));
        data.extend(iim(2, 90, b"Moscow"));
        let mut tags = Vec::new();
        parse_iim(&data, &mut tags);
        assert_eq!(tags.len(), 2);
        let keywords = tags.iter().find(|t| t.name == "Keywords").unwrap();
        assert_eq!(keywords.value.display(), "night, street");
        let city = tags.iter().find(|t| t.name == "City").unwrap();
        assert_eq!(city.value.display(), "Moscow");
        assert_eq!(city.group1, "IPTC");
    }

    #[test]
    fn extended_length_is_supported() {
        let value = vec![b'x'; 40_000];
        let mut data = vec![0x1C, 2, 120];
        data.extend_from_slice(&0x8004u16.to_be_bytes());
        data.extend_from_slice(&(value.len() as u32).to_be_bytes());
        data.extend_from_slice(&value);
        let mut tags = Vec::new();
        parse_iim(&data, &mut tags);
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn formats_iptc_dates_and_times_like_exiftool() {
        let mut data = iim(2, 55, b"20020202");
        data.extend(iim(2, 60, b"020202+0200"));
        data.extend(iim(2, 62, b"20060606"));
        data.extend(iim(2, 63, b"060606+0600"));
        let mut tags = Vec::new();
        parse_iim(&data, &mut tags);
        let value = |name: &str| {
            tags.iter()
                .find(|tag| tag.name == name)
                .map(|tag| tag.value.display())
                .unwrap()
        };
        assert_eq!(value("DateCreated"), "2002:02:02");
        assert_eq!(value("TimeCreated"), "02:02:02+02:00");
        assert_eq!(value("DigitalCreationDate"), "2006:06:06");
        assert_eq!(value("DigitalCreationTime"), "06:06:06+06:00");
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        let mut data = vec![0x1C, 2, 25, 0xFF, 0xFF];
        data.extend_from_slice(&[b'x'; 8]);
        for cut in 0..data.len() {
            parse_iim(&data[..cut], &mut tags);
            parse_photoshop(&data[..cut], &mut tags, &mut warnings);
        }
    }
}
