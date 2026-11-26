//! Minimal ISO-BMFF walker for HEIC/HEIF: locate the `Exif` item and return
//! its TIFF payload, and resolve the primary image dimensions.

#[derive(Clone, Copy, Debug)]
struct BoxRef<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

fn boxes(data: &[u8]) -> Vec<BoxRef<'_>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let kind = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let (header, size) = if size32 == 1 {
            if pos + 16 > data.len() {
                break;
            }
            let size64 = u64::from_be_bytes([
                data[pos + 8],
                data[pos + 9],
                data[pos + 10],
                data[pos + 11],
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]);
            (16usize, size64 as usize)
        } else if size32 == 0 {
            (8usize, data.len() - pos)
        } else {
            (8usize, size32 as usize)
        };
        if size < header || pos + size > data.len() {
            break;
        }
        out.push(BoxRef {
            kind,
            payload: &data[pos + header..pos + size],
        });
        pos += size;
    }
    out
}

fn find_box<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data)
        .into_iter()
        .find(|b| &b.kind == kind)
        .map(|b| b.payload)
}

/// Location of the EXIF item and its TIFF payload inside a HEIC file.
#[derive(Clone, Copy, Debug)]
pub struct ExifInfo {
    pub item_id: u32,
    pub item_start: usize,
    pub item_len: usize,
    pub tiff_start: usize,
    pub tiff_len: usize,
}

/// Locate the EXIF item using `meta`/`iloc`.
pub fn exif_info(data: &[u8]) -> Option<ExifInfo> {
    let meta = find_box(data, b"meta")?;
    let meta_payload = meta.get(4..)?;
    let exif_id = find_exif_item_id(meta_payload)?;
    let idat = find_box(meta_payload, b"idat");
    let iloc_payload = find_box(meta_payload, b"iloc")?;
    let mut locations = parse_iloc(iloc_payload)?;
    let loc = locations.remove(&exif_id)?;
    let item_start = match loc.construction_method {
        0 => loc.offset as usize,
        1 => {
            let idat = idat?;
            let idat_pos = idat.as_ptr() as usize - data.as_ptr() as usize;
            idat_pos + loc.offset as usize
        }
        _ => return None,
    };
    let item_len = loc.length as usize;
    let item = data.get(item_start..item_start.checked_add(item_len)?)?;
    if item.len() < 4 {
        return None;
    }
    let header_offset = u32::from_be_bytes([item[0], item[1], item[2], item[3]]) as usize;
    let mut tiff_start = item_start.checked_add(4 + header_offset)?;
    if data.get(tiff_start..tiff_start + 6) == Some(b"Exif\0\0") {
        tiff_start += 6;
    }
    let tiff_len = item_start.checked_add(item_len)?.saturating_sub(tiff_start);
    Some(ExifInfo {
        item_id: exif_id,
        item_start,
        item_len,
        tiff_start,
        tiff_len,
    })
}

/// Update the first extent of an `iloc` item (construction method 0 only).
pub fn set_exif_location(
    data: &mut [u8],
    item_id: u32,
    offset: u64,
    length: u64,
) -> Result<(), String> {
    let meta = find_box(data, b"meta").ok_or_else(|| "meta box not found".to_string())?;
    let meta_payload = meta
        .get(4..)
        .ok_or_else(|| "invalid meta box".to_string())?;
    let iloc_payload =
        find_box(meta_payload, b"iloc").ok_or_else(|| "iloc box not found".to_string())?;
    let iloc_start = iloc_payload.as_ptr() as usize - data.as_ptr() as usize;
    let version = *iloc_payload
        .first()
        .ok_or_else(|| "empty iloc".to_string())?;
    let mut pos = iloc_start + 4;
    let sizes = *data.get(pos).ok_or_else(|| "truncated iloc".to_string())?;
    let offset_size = (sizes >> 4) as usize;
    let length_size = (sizes & 0x0F) as usize;
    let sizes2 = *data
        .get(pos + 1)
        .ok_or_else(|| "truncated iloc".to_string())?;
    let base_offset_size = (sizes2 >> 4) as usize;
    let index_size = if version >= 1 {
        (sizes2 & 0x0F) as usize
    } else {
        0
    };
    pos += 2;
    let count = if version < 2 {
        let count = u16::from_be_bytes([
            *data.get(pos).ok_or_else(|| "truncated iloc".to_string())?,
            *data
                .get(pos + 1)
                .ok_or_else(|| "truncated iloc".to_string())?,
        ]) as usize;
        pos += 2;
        count
    } else {
        let count = u32::from_be_bytes([
            *data.get(pos).ok_or_else(|| "truncated iloc".to_string())?,
            *data
                .get(pos + 1)
                .ok_or_else(|| "truncated iloc".to_string())?,
            *data
                .get(pos + 2)
                .ok_or_else(|| "truncated iloc".to_string())?,
            *data
                .get(pos + 3)
                .ok_or_else(|| "truncated iloc".to_string())?,
        ]) as usize;
        pos += 4;
        count
    };
    let read = |data: &[u8], pos: usize, size: usize| -> Result<u64, String> {
        let bytes = data
            .get(pos..pos + size)
            .ok_or_else(|| "truncated iloc".to_string())?;
        let mut value = 0u64;
        for byte in bytes {
            value = (value << 8) | *byte as u64;
        }
        Ok(value)
    };
    for _ in 0..count {
        let id = if version < 2 {
            read(data, pos, 2)? as u32
        } else {
            read(data, pos, 4)? as u32
        };
        pos += if version < 2 { 2 } else { 4 };
        let construction = if version >= 1 {
            (read(data, pos, 2)? & 0x0F) as u16
        } else {
            0
        };
        pos += if version >= 1 { 2 } else { 0 };
        pos += 2;
        let _base = read(data, pos, base_offset_size)?;
        pos += base_offset_size;
        let extents = read(data, pos, 2)? as usize;
        pos += 2;
        for _ in 0..extents {
            if index_size > 0 {
                pos += index_size;
            }
            let offset_pos = pos;
            let _offset = read(data, pos, offset_size)?;
            pos += offset_size;
            let length_pos = pos;
            let _length = read(data, pos, length_size)?;
            pos += length_size;
            if id == item_id {
                if construction != 0 {
                    return Err("idat-relative EXIF item is not supported".to_string());
                }
                if offset_size == 0 || length_size == 0 {
                    return Err("iloc extent has no offset/length fields".to_string());
                }
                write_be(data, offset_pos, offset_size, offset)?;
                write_be(data, length_pos, length_size, length)?;
                return Ok(());
            }
        }
    }
    Err("EXIF item not found in iloc".to_string())
}

fn write_be(data: &mut [u8], pos: usize, width: usize, value: u64) -> Result<(), String> {
    if width == 0 || width > 8 || pos + width > data.len() {
        return Err("invalid iloc field".to_string());
    }
    for index in 0..width {
        data[pos + index] = (value >> (8 * (width - index - 1))) as u8;
    }
    Ok(())
}

/// Extract the EXIF payload (TIFF bytes) from a HEIC/HEIF file.
pub fn extract_exif(data: &[u8]) -> Option<&[u8]> {
    let meta = find_box(data, b"meta")?;
    let meta_payload = meta.get(4..)?;

    let exif_id = find_exif_item_id(meta_payload)?;
    let idat = find_box(meta_payload, b"idat");

    let iloc_payload = find_box(meta_payload, b"iloc")?;
    let mut locations = parse_iloc(iloc_payload)?;
    let loc = locations.remove(&exif_id)?;

    let slice = match loc.construction_method {
        0 => data.get(loc.offset as usize..(loc.offset + loc.length) as usize),
        1 => idat?.get(loc.offset as usize..(loc.offset + loc.length) as usize),
        _ => None,
    }?;

    // Exif item: u32 offset to the TIFF header, then the payload.
    if slice.len() < 4 {
        return None;
    }
    let header_offset = u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]) as usize;
    let mut tiff = slice.get(4 + header_offset..)?;
    if tiff.starts_with(b"Exif\0\0") {
        tiff = &tiff[6..];
    }
    if tiff.starts_with(b"II*") || tiff.starts_with(b"MM\0*") {
        Some(tiff)
    } else {
        None
    }
}

fn find_exif_item_id(meta: &[u8]) -> Option<u32> {
    let iinf = find_box(meta, b"iinf")?;
    let version = *iinf.first()?;
    let payload = iinf.get(4..)?;
    let (count, mut pos) = if version == 0 {
        if payload.len() < 2 {
            return None;
        }
        (
            u16::from_be_bytes([payload[0], payload[1]]) as usize,
            2usize,
        )
    } else {
        if payload.len() < 4 {
            return None;
        }
        (
            u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize,
            4usize,
        )
    };
    for _ in 0..count {
        let infe = boxes(payload.get(pos..)?).into_iter().next()?;
        if &infe.kind != b"infe" {
            break;
        }
        let entry = infe.payload;
        let version = *entry.first()?;
        let body = entry.get(4..)?;
        let (item_id, type_pos) = match version {
            0 | 1 => {
                if body.len() < 2 {
                    return None;
                }
                (u16::from_be_bytes([body[0], body[1]]) as u32, None)
            }
            2 => {
                if body.len() < 8 {
                    return None;
                }
                (u16::from_be_bytes([body[0], body[1]]) as u32, Some(4usize))
            }
            3 => {
                if body.len() < 10 {
                    return None;
                }
                (
                    u32::from_be_bytes([body[0], body[1], body[2], body[3]]),
                    Some(6usize),
                )
            }
            _ => return None,
        };
        if let Some(type_pos) = type_pos
            && body.get(type_pos..type_pos + 4) == Some(b"Exif")
        {
            return Some(item_id);
        }
        pos += 8 + infe.payload.len();
    }
    None
}

struct ItemLoc {
    offset: u64,
    length: u64,
    construction_method: u16,
}

fn parse_iloc(data: &[u8]) -> Option<std::collections::HashMap<u32, ItemLoc>> {
    let version = *data.first()?;
    let mut pos = 4usize;
    let sizes = *data.get(pos)?;
    let offset_size = (sizes >> 4) as usize;
    let length_size = (sizes & 0x0F) as usize;
    let sizes2 = *data.get(pos + 1)?;
    let base_offset_size = (sizes2 >> 4) as usize;
    let index_size = if version >= 1 {
        (sizes2 & 0x0F) as usize
    } else {
        0
    };
    pos += 2;

    let count = if version < 2 {
        let count = u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]) as usize;
        pos += 2;
        count
    } else {
        let count = u32::from_be_bytes([
            *data.get(pos)?,
            *data.get(pos + 1)?,
            *data.get(pos + 2)?,
            *data.get(pos + 3)?,
        ]) as usize;
        pos += 4;
        count
    };

    let read = |pos: &mut usize, size: usize| -> Option<u64> {
        if size == 0 {
            return Some(0);
        }
        let bytes = data.get(*pos..*pos + size)?;
        let mut value: u64 = 0;
        for byte in bytes {
            value = (value << 8) | *byte as u64;
        }
        *pos += size;
        Some(value)
    };

    let mut map = std::collections::HashMap::new();
    for _ in 0..count {
        let item_id = if version < 2 {
            read(&mut pos, 2)? as u32
        } else {
            read(&mut pos, 4)? as u32
        };
        let construction_method = if version >= 1 {
            (read(&mut pos, 2)? & 0x0F) as u16
        } else {
            0
        };
        let _data_reference = read(&mut pos, 2)?;
        let base_offset = read(&mut pos, base_offset_size)?;
        let extent_count = read(&mut pos, 2)?;
        let mut offset = base_offset;
        let mut length = 0u64;
        for _ in 0..extent_count {
            if index_size > 0 {
                let _index = read(&mut pos, index_size)?;
            }
            let extent_offset = read(&mut pos, offset_size)?;
            offset = base_offset.saturating_add(extent_offset);
            length = length.saturating_add(read(&mut pos, length_size)?);
        }
        map.entry(item_id).or_insert(ItemLoc {
            offset,
            length,
            construction_method,
        });
    }
    Some(map)
}

/// Extract the XMP packet (RDF/XML) from a HEIC/HEIF file.
pub fn extract_xmp(data: &[u8]) -> Option<&[u8]> {
    extract_xmps(data).into_iter().next()
}

/// All XMP packets in the file, ordered by item id (main packet first).
pub fn extract_xmps(data: &[u8]) -> Vec<&[u8]> {
    let mut packets = Vec::new();
    let Some(meta) = find_box(data, b"meta") else {
        return packets;
    };
    let Some(meta_payload) = meta.get(4..) else {
        return packets;
    };
    let Some(iloc_payload) = find_box(meta_payload, b"iloc") else {
        return packets;
    };
    let Some(locations) = parse_iloc(iloc_payload) else {
        return packets;
    };
    let idat = find_box(meta_payload, b"idat");
    let mut ids: Vec<u32> = locations.keys().copied().collect();
    ids.sort();
    for id in ids {
        let location = &locations[&id];
        let slice = match location.construction_method {
            0 => data.get(location.offset as usize..(location.offset + location.length) as usize),
            1 => idat.and_then(|idat| {
                idat.get(location.offset as usize..(location.offset + location.length) as usize)
            }),
            _ => None,
        };
        let Some(slice) = slice else {
            continue;
        };
        for marker in [b"<?xpacket".as_slice(), b"<x:xmpmeta".as_slice()] {
            if let Some(position) = find_subslice(slice, marker) {
                packets.push(&slice[position..]);
                break;
            }
        }
    }
    packets
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Dimensions of the primary image, resolved through `pitm` + `ipma`/`ispe`.
pub fn dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let meta = find_box(data, b"meta")?;
    let meta_payload = meta.get(4..)?;
    let iprp = find_box(meta_payload, b"iprp")?;
    let ipco = find_box(iprp, b"ipco")?;
    let properties = boxes(ipco);

    if let Some(primary) = primary_item_id(meta_payload)
        && let Some(associations) = ipma_associations(meta_payload, primary)
    {
        for index in associations {
            if let Some(property) = properties.get(index.saturating_sub(1) as usize)
                && &property.kind == b"ispe"
                && property.payload.len() >= 12
            {
                return parse_ispe(property.payload);
            }
        }
    }

    properties
        .iter()
        .filter(|property| &property.kind == b"ispe")
        .filter_map(|property| parse_ispe(property.payload))
        .max_by_key(|(width, height)| *width as u64 * *height as u64)
}

fn parse_ispe(payload: &[u8]) -> Option<(u32, u32)> {
    if payload.len() < 12 {
        return None;
    }
    let width = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let height = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
    Some((width, height))
}

fn primary_item_id(meta: &[u8]) -> Option<u32> {
    let pitm = find_box(meta, b"pitm")?;
    let version = *pitm.first()?;
    if version == 0 {
        pitm.get(4..6)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]) as u32)
    } else {
        pitm.get(4..8)
            .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

fn ipma_associations(meta: &[u8], item: u32) -> Option<Vec<u16>> {
    let ipma = find_box(meta, b"ipma")?;
    let version = *ipma.first()?;
    let flags = u32::from_be_bytes([ipma[0], ipma[1], ipma[2], ipma[3]]) & 0x00FF_FFFF;
    if ipma.len() < 8 {
        return None;
    }
    let count = u32::from_be_bytes([ipma[4], ipma[5], ipma[6], ipma[7]]) as usize;
    let mut pos = 8usize;
    let mut result = Vec::new();
    for _ in 0..count {
        let id = if version < 1 {
            let bytes = ipma.get(pos..pos + 2)?;
            pos += 2;
            u16::from_be_bytes([bytes[0], bytes[1]]) as u32
        } else {
            let bytes = ipma.get(pos..pos + 4)?;
            pos += 4;
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        };
        let association_count = *ipma.get(pos)? as usize;
        pos += 1;
        for _ in 0..association_count {
            let association = if flags & 1 == 1 {
                let bytes = ipma.get(pos..pos + 2)?;
                pos += 2;
                u16::from_be_bytes([bytes[0], bytes[1]])
            } else {
                let byte = *ipma.get(pos)?;
                pos += 1;
                byte as u16
            };
            if id == item {
                result.push(association & 0x7FFF);
            }
        }
    }
    Some(result)
}

use crate::model::{Tag, Value};

fn be16_at(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)
        .map(|raw| u16::from_be_bytes([raw[0], raw[1]]))
}

fn be32_at(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .map(|raw| u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn compatibilities(flags: u32) -> String {
    let mut names = Vec::new();
    if flags & 0x4000_0000 != 0 {
        names.push("Main Still Picture");
    }
    if flags & 0x2000_0000 != 0 {
        names.push("Main 10");
    }
    if flags & 0x1000_0000 != 0 {
        names.push("Main");
    }
    if names.is_empty() {
        flags.to_string()
    } else {
        names.join(", ")
    }
}

fn primary_properties(iprp: &[u8], primary_id: u32) -> Option<Vec<usize>> {
    let ipma = find_box(iprp, b"ipma")?;
    let version = *ipma.first()?;
    let flags = u32::from_be_bytes([0, ipma[1], ipma[2], ipma[3]]);
    let count = be32_at(ipma, 4)? as usize;
    let mut pos = 8usize;
    for _ in 0..count {
        let item = if version < 1 {
            let value = be16_at(ipma, pos)? as u32;
            pos += 2;
            value
        } else {
            let value = be32_at(ipma, pos)?;
            pos += 4;
            value
        };
        let associations = *ipma.get(pos)? as usize;
        pos += 1;
        let mut indices = Vec::with_capacity(associations);
        for _ in 0..associations {
            let value = if flags & 1 != 0 {
                let value = be16_at(ipma, pos)? as usize;
                pos += 2;
                value & 0x7FFF
            } else {
                let value = *ipma.get(pos)? as usize;
                pos += 1;
                value & 0x7F
            };
            indices.push(value);
        }
        if item == primary_id {
            return Some(indices);
        }
    }
    None
}

/// QuickTime/HEIF metadata that lives in ISO-BMFF boxes of a HEIC container.
pub fn quicktime_tags(data: &[u8], out: &mut Vec<Tag>) {
    let top = boxes(data);
    if let Some(ftyp) = top.iter().find(|b| &b.kind == b"ftyp") {
        let payload = ftyp.payload;
        let major = payload.get(0..4).unwrap_or_default();
        let label = match major {
            b"heic" => "High Efficiency Image Format HEVC still image (.HEIC)",
            b"heix" => "High Efficiency Image Format HEVC image (.HEIX)",
            b"mif1" => "High Efficiency Image Format still image (.HEIF)",
            _ => "Unknown",
        };
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MajorBrand",
            Value::Str(label.to_string()),
        ));
        let minor = be32_at(payload, 4).unwrap_or(0);
        let minor = if minor == 0 {
            "0.0.0".to_string()
        } else {
            format!(
                "{}.{}.{}",
                (minor >> 16) & 0xFF,
                (minor >> 8) & 0xFF,
                minor & 0xFF
            )
        };
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MinorVersion",
            Value::Str(minor),
        ));
        let brands: Vec<String> = payload
            .get(8..)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| String::from_utf8_lossy(chunk).trim_end().to_string())
            .collect();
        if !brands.is_empty() {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "CompatibleBrands",
                Value::Str(brands.join(", ")),
            ));
        }
    }

    // mdat header/size: recompute from the original buffer.
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = be32_at(data, pos).unwrap_or(0);
        let kind = data.get(pos + 4..pos + 8).unwrap_or_default();
        let (header, size) = if size32 == 1 {
            (
                16usize,
                data.get(pos + 8..pos + 16)
                    .map(|raw| {
                        u64::from_be_bytes([
                            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                        ])
                    })
                    .unwrap_or(0) as usize,
            )
        } else if size32 == 0 {
            (8usize, data.len().saturating_sub(pos))
        } else {
            (8usize, size32 as usize)
        };
        if kind == b"mdat" && size >= header {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MediaDataSize",
                Value::Int((size - header) as i64),
            ));
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MediaDataOffset",
                Value::Int((pos + header) as i64),
            ));
        }
        if size < header {
            break;
        }
        pos += size;
    }

    let Some(meta) = top.iter().find(|b| &b.kind == b"meta") else {
        return;
    };
    let meta = meta.payload.get(4..).unwrap_or_default();

    if let Some(hdlr) = find_box(meta, b"hdlr")
        && let Some(kind) = hdlr.get(8..12)
    {
        let label = match kind {
            b"pict" => "Picture",
            b"vide" => "Video Track",
            _ => "Unknown",
        };
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "HandlerType",
            Value::Str(label.to_string()),
        ));
    }

    let mut primary_id = None;
    if let Some(pitm) = find_box(meta, b"pitm") {
        let version = pitm.first().copied().unwrap_or(0);
        let id = if version == 0 {
            be16_at(pitm, 4).map(u32::from)
        } else {
            be32_at(pitm, 4)
        };
        if let Some(id) = id {
            primary_id = Some(id);
            out.push(Tag::new(
                "Meta",
                "Meta",
                "PrimaryItemReference",
                Value::Int(id as i64),
            ));
        }
    }

    let Some(iprp) = find_box(meta, b"iprp") else {
        return;
    };
    let Some(ipco) = find_box(iprp, b"ipco") else {
        return;
    };
    let properties = boxes(ipco);
    let selected = primary_id
        .and_then(|id| primary_properties(iprp, id))
        .unwrap_or_default();
    let first = |kind: &[u8; 4]| {
        properties
            .iter()
            .enumerate()
            .find(|(index, b)| {
                &b.kind == kind && (selected.is_empty() || selected.contains(&(index + 1)))
            })
            .or_else(|| properties.iter().enumerate().find(|(_, b)| &b.kind == kind))
            .map(|(_, b)| b.payload)
    };

    if let Some(clli) = first(b"clli") {
        if let Some(max_content) = be16_at(clli, 0) {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MaxContentLightLevel",
                Value::Int(max_content as i64),
            ));
        }
        if let Some(max_pic) = be16_at(clli, 2) {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MaxPicAverageLightLevel",
                Value::Int(max_pic as i64),
            ));
        }
    }
    if let Some(ispe) = first(b"ispe")
        && let (Some(width), Some(height)) = (be32_at(ispe, 4), be32_at(ispe, 8))
    {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "ImageSpatialExtent",
            Value::Str(format!("{width}x{height}")),
        ));
    }
    if let Some(irot) = first(b"irot") {
        let angle = irot.first().copied().unwrap_or(0) & 0x03;
        let label = match angle {
            0 => "Horizontal (normal)",
            1 => "Rotate 270 CW",
            2 => "Rotate 180",
            _ => "Rotate 90 CW",
        };
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "Rotation",
                Value::Int(angle as i64),
            )
            .with_print(label),
        );
    }
    if let Some(pixi) = first(b"pixi") {
        let count = pixi.get(4).copied().unwrap_or(0) as usize;
        let depths: Vec<String> = pixi
            .get(5..)
            .unwrap_or_default()
            .iter()
            .take(count)
            .map(u8::to_string)
            .collect();
        if !depths.is_empty() {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "ImagePixelDepth",
                Value::Str(depths.join(" ")),
            ));
        }
    }
    if let Some(auxc) = first(b"auxC") {
        let text = auxc.get(4..).unwrap_or_default();
        let end = text
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(text.len());
        if end > 0 {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "AuxiliaryImageType",
                Value::Str(String::from_utf8_lossy(&text[..end]).to_string()),
            ));
        }
    }
    for colr in properties.iter().filter(|b| &b.kind == b"colr") {
        if colr.payload.get(0..4) != Some(b"nclx") {
            continue;
        }
        let payload = colr.payload;
        let primaries = be16_at(payload, 4).unwrap_or(2);
        let transfer = be16_at(payload, 6).unwrap_or(2);
        let matrix = be16_at(payload, 8).unwrap_or(2);
        let full_range = payload.get(10).copied().unwrap_or(0) & 0x80 != 0;
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "ColorProfiles",
            Value::Str("nclx".to_string()),
        ));
        for (name, value) in [
            ("ColorPrimaries", primaries),
            ("TransferCharacteristics", transfer),
            ("MatrixCoefficients", matrix),
        ] {
            let label = match value {
                1 => "BT.709",
                2 => "Unspecified",
                5 => "BT.470 System B/G",
                6 => "BT.601",
                9 => "BT.2020",
                _ => "Unknown",
            };
            out.push(
                Tag::new("QuickTime", "QuickTime", name, Value::Int(value as i64))
                    .with_print(label),
            );
        }
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "VideoFullRangeFlag",
                Value::Int(full_range as i64),
            )
            .with_print(if full_range { "Full" } else { "Limited" }),
        );
        break;
    }
    if let Some(hvcc) = first(b"hvcC") {
        hevc_config(hvcc, out);
    }
}

fn hevc_config(payload: &[u8], out: &mut Vec<Tag>) {
    let Some(version) = payload.first().copied() else {
        return;
    };
    out.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "HEVCConfigurationVersion",
        Value::Int(version as i64),
    ));
    let Some(profile_byte) = payload.get(1).copied() else {
        return;
    };
    let space = (profile_byte >> 6) & 0x03;
    let tier = (profile_byte >> 5) & 0x01;
    let profile = profile_byte & 0x1F;
    out.push(
        Tag::new(
            "QuickTime",
            "QuickTime",
            "GeneralProfileSpace",
            Value::Int(space as i64),
        )
        .with_print(match space {
            0 => "Conforming",
            1 => "Reserved",
            2 => "Reserved",
            _ => "Reserved",
        }),
    );
    out.push(
        Tag::new(
            "QuickTime",
            "QuickTime",
            "GeneralTierFlag",
            Value::Int(tier as i64),
        )
        .with_print(if tier == 0 { "Main Tier" } else { "High Tier" }),
    );
    out.push(
        Tag::new(
            "QuickTime",
            "QuickTime",
            "GeneralProfileIDC",
            Value::Int(profile as i64),
        )
        .with_print(match profile {
            1 => "Main",
            2 => "Main 10",
            3 => "Main Still Picture",
            4 => "Format Range Extensions",
            _ => "Unknown",
        }),
    );
    if let Some(flags) = be32_at(payload, 2) {
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "GenProfileCompatibilityFlags",
                Value::Int(flags as i64),
            )
            .with_print(compatibilities(flags)),
        );
    }
    if let Some(raw) = payload.get(6..12) {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "ConstraintIndicatorFlags",
            Value::Str(raw.iter().map(u8::to_string).collect::<Vec<_>>().join(" ")),
        ));
    }
    if let Some(level) = payload.get(12).copied() {
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "GeneralLevelIDC",
                Value::Int(level as i64),
            )
            .with_print(format!("{level} (level {:.1})", level as f64 / 30.0)),
        );
    }
    if let Some(min_spatial) = be16_at(payload, 13) {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MinSpatialSegmentationIDC",
            Value::Int((min_spatial & 0x0FFF) as i64),
        ));
    }
    if let Some(parallelism) = payload.get(15).copied() {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "ParallelismType",
            Value::Int((parallelism & 0x03) as i64),
        ));
    }
    if let Some(chroma) = payload.get(16).copied() {
        let chroma = chroma & 0x03;
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "ChromaFormat",
                Value::Int(chroma as i64),
            )
            .with_print(match chroma {
                0 => "Monochrome",
                1 => "4:2:0",
                2 => "4:2:2",
                _ => "4:4:4",
            }),
        );
    }
    if let Some(luma) = payload.get(17).copied() {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "BitDepthLuma",
            Value::Int(((luma & 0x07) + 8) as i64),
        ));
    }
    if let Some(chroma) = payload.get(18).copied() {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "BitDepthChroma",
            Value::Int(((chroma & 0x07) + 8) as i64),
        ));
    }
    if let Some(rate) = be16_at(payload, 19) {
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "AverageFrameRate",
            Value::Int(rate as i64),
        ));
    }
    if let Some(flags) = payload.get(21).copied() {
        let constant = (flags >> 6) & 0x03;
        let layers = (flags >> 3) & 0x07;
        let nested = (flags >> 2) & 0x01;
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "ConstantFrameRate",
                Value::Int(constant as i64),
            )
            .with_print(match constant {
                0 => "Unknown",
                1 => "Constant",
                2 => "Variable",
                _ => "Unknown",
            }),
        );
        out.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "NumTemporalLayers",
            Value::Int(layers as i64),
        ));
        out.push(
            Tag::new(
                "QuickTime",
                "QuickTime",
                "TemporalIDNested",
                Value::Int(nested as i64),
            )
            .with_print(if nested == 0 { "No" } else { "Yes" }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn resolves_primary_dimensions() {
        let ispe = iso_box(b"ispe", &[0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 20]);
        let ipco = iso_box(b"ipco", &ispe);
        let iprp = iso_box(b"iprp", &ipco);
        let pitm = iso_box(b"pitm", &[0, 0, 0, 0, 0, 1]);
        let ipma = iso_box(b"ipma", &[0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0x81]);
        let mut meta_payload = vec![0u8, 0, 0, 0];
        meta_payload.extend_from_slice(&pitm);
        meta_payload.extend_from_slice(&iprp);
        meta_payload.extend_from_slice(&ipma);
        let meta = iso_box(b"meta", &meta_payload);
        let file = iso_box(b"ftyp", b"heic\0\0\0\0mif1heic");
        let mut data = file;
        data.extend_from_slice(&meta);
        assert_eq!(dimensions(&data), Some((10, 20)));
    }

    #[test]
    fn truncated_input_is_safe() {
        let data = [0, 0, 0, 16, b'm', b'e', b't', b'a', 0, 0];
        for cut in 0..data.len() {
            let _ = extract_exif(&data[..cut]);
            let _ = dimensions(&data[..cut]);
        }
    }
}
