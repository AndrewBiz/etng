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
