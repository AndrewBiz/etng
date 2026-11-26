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
