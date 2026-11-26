//! MakerNote subset for Canon, Nikon and Sony.
//!
//! Vendor notes are plain IFD blocks with different preambles. We try a few
//! well-known layouts, keep the one that resolves the most known tags, and map
//! the result through the generated `-listx` tables.

use etng_tags::tables;

use crate::model::{Tag, Value};
use crate::tiff::{Entry, Tiff};

struct Layout {
    ifd_offset: u32,
    base_for_values: usize,
    le: bool,
}

pub fn parse(
    make: Option<&str>,
    note: &[u8],
    file_le: bool,
    out: &mut Vec<Tag>,
    warnings: &mut Vec<String>,
    numeric: bool,
) {
    let Some(vendor) = vendor_of(make) else {
        warnings.push("MakerNotes: unsupported vendor".to_string());
        return;
    };
    let table = match vendor {
        "Canon" => "Canon::Main",
        "Nikon" => "Nikon::Main",
        "Sony" => "Sony::Main",
        "Apple" => "Apple::Main",
        _ => return,
    };

    let layouts = candidate_layouts(vendor, note, file_le);
    let mut best: Option<Vec<Tag>> = None;
    let mut best_count = 0usize;

    for layout in layouts {
        let tags = try_layout(note, &layout, table, vendor, numeric);
        let recognized = tags.len();
        if recognized > best_count {
            best_count = recognized;
            best = Some(tags);
        }
    }

    match best {
        Some(tags) if !tags.is_empty() => out.extend(tags),
        _ => warnings.push(format!("MakerNotes: could not locate {vendor} IFD")),
    }
}

/// Parse a Canon MakerNote that lives inside a TIFF payload at `ifd_offset`.
/// Canon value offsets are relative to the TIFF header, not the note block.
pub fn parse_canon_at(
    payload: &[u8],
    ifd_offset: u32,
    out: &mut Vec<Tag>,
    warnings: &mut Vec<String>,
    numeric: bool,
) -> bool {
    let Some(tiff) = Tiff::new(payload) else {
        warnings.push("MakerNotes: Canon TIFF header missing".to_string());
        return false;
    };
    let Some(ifd) = tiff.read_ifd(ifd_offset) else {
        warnings.push("MakerNotes: Canon IFD missing".to_string());
        return false;
    };
    let mut count = 0;
    for entry in &ifd.entries {
        let id = entry.tag.to_string();
        let Some(def) = tables().find_table_id("Canon::Main", &id) else {
            continue;
        };
        let Some(value) = entry_value(&tiff, entry) else {
            continue;
        };
        if matches!(value, Value::Bytes(_)) {
            continue;
        }
        let mut tag = Tag::new("MakerNotes", "Canon", def.name, value);
        if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
            tag = tag.with_print(print);
        }
        out.push(tag);
        count += 1;
    }
    crate::cr3::canon_tables(payload, ifd_offset, out, numeric);
    crate::cr3::canon_fixups(out, numeric);
    count > 0
}

fn vendor_of(make: Option<&str>) -> Option<&'static str> {
    let make = make?.to_ascii_uppercase();
    if make.contains("CANON") {
        Some("Canon")
    } else if make.contains("NIKON") {
        Some("Nikon")
    } else if make.contains("SONY") {
        Some("Sony")
    } else if make.contains("APPLE") {
        Some("Apple")
    } else {
        None
    }
}

fn candidate_layouts(vendor: &str, note: &[u8], file_le: bool) -> Vec<Layout> {
    let mut layouts = vec![Layout {
        ifd_offset: 0,
        base_for_values: 0,
        le: file_le,
    }];
    match vendor {
        "Canon" => {
            if let Some(tiff) = Tiff::at(note, 0)
                && let Some(offset) = tiff.first_ifd_offset()
            {
                layouts.push(Layout {
                    ifd_offset: offset,
                    base_for_values: 0,
                    le: tiff.le,
                });
            }
        }
        "Nikon" => {
            if note.starts_with(b"Nikon\0")
                && let Some(tiff) = Tiff::at(note, 10)
                && let Some(offset) = tiff.first_ifd_offset()
            {
                layouts.push(Layout {
                    ifd_offset: offset,
                    base_for_values: 10,
                    le: tiff.le,
                });
            }
        }
        "Sony" if note.starts_with(b"SONY") => {
            layouts.push(Layout {
                ifd_offset: 12,
                base_for_values: 0,
                le: true,
            });
            layouts.push(Layout {
                ifd_offset: 12,
                base_for_values: 12,
                le: true,
            });
        }
        "Apple" if note.starts_with(b"Apple iOS\0") => {
            // Header: "Apple iOS\0" + version, then a big-endian IFD whose
            // entries start at offset 12 (`MM` marks the byte order).
            layouts.push(Layout {
                ifd_offset: 12,
                base_for_values: 0,
                le: false,
            });
            layouts.push(Layout {
                ifd_offset: 14,
                base_for_values: 0,
                le: false,
            });
        }
        _ => {}
    }
    layouts
}

fn try_layout(
    note: &[u8],
    layout: &Layout,
    table: &str,
    vendor: &'static str,
    numeric: bool,
) -> Vec<Tag> {
    let tiff = Tiff::from_ifd(note, layout.base_for_values, layout.le);
    let Some(ifd) = tiff.read_ifd(layout.ifd_offset) else {
        return Vec::new();
    };
    let mut tags = Vec::new();
    for entry in &ifd.entries {
        let id = entry.tag.to_string();
        let Some(def) = tables().find_table_id(table, &id) else {
            continue;
        };
        let Some(value) = entry_value(&tiff, entry) else {
            continue;
        };
        if matches!(value, Value::Bytes(_)) {
            // Complex vendor blobs (plists, matrices) are out of the subset.
            continue;
        }
        let mut tag = Tag::new("MakerNotes", vendor, def.name, value);
        if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
            tag = tag.with_print(print);
        }
        tags.push(tag);
    }
    tags
}

pub(crate) fn entry_value(tiff: &Tiff<'_>, entry: &Entry) -> Option<Value> {
    let bytes = tiff.entry_bytes(entry)?;
    match entry.typ {
        2 => {
            let text = String::from_utf8_lossy(bytes);
            Some(Value::Str(text.trim_end_matches('\0').trim().to_string()))
        }
        1 | 6 => Some(int_from(bytes, entry.count as usize, 1, tiff.le)),
        3 | 8 => Some(int_from(bytes, entry.count as usize, 2, tiff.le)),
        4 | 9 | 13 => Some(int_from(bytes, entry.count as usize, 4, tiff.le)),
        5 | 10 => {
            let first = bytes.get(0..8)?;
            let num = if tiff.le {
                u32::from_le_bytes([first[0], first[1], first[2], first[3]])
            } else {
                u32::from_be_bytes([first[0], first[1], first[2], first[3]])
            };
            let den = if tiff.le {
                u32::from_le_bytes([first[4], first[5], first[6], first[7]])
            } else {
                u32::from_be_bytes([first[4], first[5], first[6], first[7]])
            };
            Some(Value::Rational {
                num: num as i64,
                den: den as i64,
            })
        }
        7 => Some(Value::Bytes(bytes.to_vec())),
        _ => None,
    }
}

fn int_from(bytes: &[u8], count: usize, width: usize, le: bool) -> Value {
    let count = count.min(bytes.len() / width).max(1);
    if count == 1 {
        return Value::Int(read_int(bytes, width, le));
    }
    Value::Str(
        (0..count)
            .map(|i| read_int(&bytes[i * width..], width, le).to_string())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn read_int(bytes: &[u8], width: usize, le: bool) -> i64 {
    match width {
        1 => bytes[0] as i64,
        2 => {
            let pair = [bytes[0], bytes[1]];
            if le {
                i16::from_le_bytes(pair) as i64
            } else {
                i16::from_be_bytes(pair) as i64
            }
        }
        _ => {
            let quad = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if le {
                i32::from_le_bytes(quad) as i64
            } else {
                i32::from_be_bytes(quad) as i64
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon_ifd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        data.extend_from_slice(&16u16.to_le_bytes()); // CanonModelID
        data.extend_from_slice(&4u16.to_le_bytes()); // LONG
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0x8000_0000u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // next
        data
    }

    #[test]
    fn parses_canon_main_ifd() {
        let data = canon_ifd();
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        parse(Some("Canon"), &data, true, &mut tags, &mut warnings, false);
        assert!(!tags.is_empty(), "warnings: {warnings:?}");
        assert!(tags.iter().all(|t| t.group1 == "Canon"));
    }

    #[test]
    fn unknown_vendor_warns() {
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        parse(
            Some("PENTAX"),
            &[0, 0],
            true,
            &mut tags,
            &mut warnings,
            false,
        );
        assert!(tags.is_empty());
        assert_eq!(warnings.len(), 1);
    }
}
