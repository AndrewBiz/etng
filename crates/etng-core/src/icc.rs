//! ICC profile parsing for JPEG APP2, TIFF tag 0x8773 and HEIC `colr`.

use crate::model::{Tag, Value};
use crate::tiff::Tiff;

pub fn parse_jpeg(data: &[u8], out: &mut Vec<Tag>) {
    if let Some(profile) = from_jpeg(data) {
        parse(&profile, out);
    }
}

pub fn parse_tiff(data: &[u8], out: &mut Vec<Tag>) {
    if let Some(profile) = from_tiff(data) {
        parse(profile, out);
    }
}

pub fn parse_heic(data: &[u8], out: &mut Vec<Tag>) {
    if let Some(profile) = from_heic(data) {
        parse(profile, out);
    }
}

fn from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    let mut chunks: Vec<Option<Vec<u8>>> = Vec::new();
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let mut marker = data[pos + 1];
        let mut marker_pos = pos;
        while marker == 0xFF && marker_pos + 2 < data.len() {
            marker_pos += 1;
            marker = data[marker_pos + 1];
        }
        if matches!(marker, 0xD8 | 0xD9 | 0x01 | 0xD0..=0xD7) {
            pos = marker_pos + 2;
            continue;
        }
        if marker == 0xDA {
            break;
        }
        let length =
            u16::from_be_bytes([*data.get(marker_pos + 2)?, *data.get(marker_pos + 3)?]) as usize;
        if length < 2 || marker_pos + 2 + length > data.len() {
            break;
        }
        let payload = &data[marker_pos + 4..marker_pos + 2 + length];
        if marker == 0xE2 && payload.starts_with(b"ICC_PROFILE\0") && payload.len() >= 14 {
            let sequence = payload[12] as usize;
            let count = payload[13] as usize;
            if sequence >= 1 && sequence <= count && count <= 255 {
                if chunks.len() < count {
                    chunks.resize(count, None);
                }
                chunks[sequence - 1] = Some(payload[14..].to_vec());
            }
        }
        pos = marker_pos + 2 + length;
    }
    if chunks.is_empty() || chunks.iter().any(Option::is_none) {
        return None;
    }
    Some(chunks.into_iter().flatten().flatten().collect())
}

fn from_tiff(data: &[u8]) -> Option<&[u8]> {
    let tiff = Tiff::new(data)?;
    let ifd = tiff.read_ifd(tiff.first_ifd_offset()?)?;
    let entry = ifd.entries.iter().find(|entry| entry.tag == 0x8773)?;
    tiff.entry_bytes(entry)
}

fn from_heic(data: &[u8]) -> Option<&[u8]> {
    let mut pos = 4usize;
    while pos + 12 <= data.len() {
        if &data[pos..pos + 4] == b"colr" {
            let size =
                u32::from_be_bytes([data[pos - 4], data[pos - 3], data[pos - 2], data[pos - 1]])
                    as usize;
            if size >= 12 && pos + size <= data.len() {
                let payload = &data[pos + 4..pos + size];
                if payload.get(0..4) == Some(b"prof") {
                    return Some(&payload[4..]);
                }
            }
        }
        pos += 1;
    }
    None
}

pub fn parse(profile: &[u8], out: &mut Vec<Tag>) -> bool {
    if profile.len() < 132 || profile.get(36..40) != Some(b"acsp") {
        return false;
    }
    let be16 = |offset: usize| {
        profile
            .get(offset..offset + 2)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
    };
    let be32 = |offset: usize| {
        profile
            .get(offset..offset + 4)
            .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let be64 = |offset: usize| {
        profile.get(offset..offset + 8).map(|bytes| {
            u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        })
    };
    let add = |out: &mut Vec<Tag>, group: &'static str, name: &'static str, value: Value| {
        out.push(Tag::new("ICC_Profile", group, name, value));
    };

    let signature = |offset: usize| {
        profile
            .get(offset..offset + 4)
            .map(|bytes| {
                String::from_utf8_lossy(bytes)
                    .trim_end_matches('\0')
                    .to_string()
            })
            .unwrap_or_default()
    };

    add(
        out,
        "ICC-header",
        "ProfileCMMType",
        Value::Str(company(&signature(4))),
    );
    add(
        out,
        "ICC-header",
        "ProfileVersion",
        Value::Str(format!(
            "{}.{}.{}",
            profile[8],
            profile[9] >> 4,
            profile[9] & 0x0F
        )),
    );
    add(
        out,
        "ICC-header",
        "ProfileClass",
        Value::Str(profile_class(&signature(12))),
    );
    add(
        out,
        "ICC-header",
        "ColorSpaceData",
        Value::Str(signature(16)),
    );
    add(
        out,
        "ICC-header",
        "ProfileConnectionSpace",
        Value::Str(signature(20)),
    );
    add(
        out,
        "ICC-header",
        "ProfileDateTime",
        Value::Str(format!(
            "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
            be16(24).unwrap_or(0),
            be16(26).unwrap_or(0),
            be16(28).unwrap_or(0),
            be16(30).unwrap_or(0),
            be16(32).unwrap_or(0),
            be16(34).unwrap_or(0)
        )),
    );
    add(
        out,
        "ICC-header",
        "ProfileFileSignature",
        Value::Str(signature(36)),
    );
    add(
        out,
        "ICC-header",
        "PrimaryPlatform",
        Value::Str(company(&signature(40))),
    );
    let flags = be32(44).unwrap_or(0);
    let mut flag_names = vec![if flags & 1 != 0 {
        "Embedded"
    } else {
        "Not Embedded"
    }];
    flag_names.push(if flags & 2 != 0 {
        "Dependent"
    } else {
        "Independent"
    });
    add(
        out,
        "ICC-header",
        "CMMFlags",
        Value::Str(flag_names.join(", ")),
    );
    add(
        out,
        "ICC-header",
        "DeviceManufacturer",
        Value::Str(company(&signature(48))),
    );
    add(out, "ICC-header", "DeviceModel", Value::Str(signature(52)));
    let attributes = be64(56).unwrap_or(0);
    let mut attr_names = vec![if attributes & 1 != 0 {
        "Transparency"
    } else {
        "Reflective"
    }];
    attr_names.push(if attributes & 2 != 0 {
        "Matte"
    } else {
        "Glossy"
    });
    attr_names.push(if attributes & 4 != 0 {
        "Negative"
    } else {
        "Positive"
    });
    attr_names.push(if attributes & 8 != 0 { "B&W" } else { "Color" });
    add(
        out,
        "ICC-header",
        "DeviceAttributes",
        Value::Str(attr_names.join(", ")),
    );
    add(
        out,
        "ICC-header",
        "RenderingIntent",
        Value::Str(
            match be32(64).unwrap_or(0) {
                0 => "Perceptual",
                1 => "Media-Relative Colorimetric",
                2 => "Saturation",
                3 => "Absolute Colorimetric",
                _ => "Unknown",
            }
            .to_string(),
        ),
    );
    add(
        out,
        "ICC-header",
        "ConnectionSpaceIlluminant",
        Value::Str(xyz(profile, 68, 12)),
    );
    add(
        out,
        "ICC-header",
        "ProfileCreator",
        Value::Str(company(&signature(80))),
    );
    let id = profile.get(84..100).unwrap_or_default();
    let profile_id = if id.iter().all(|byte| *byte == 0) {
        Value::Int(0)
    } else {
        Value::Str(id.iter().map(|byte| format!("{byte:02x}")).collect())
    };
    add(out, "ICC-header", "ProfileID", profile_id);

    let count = be32(128).unwrap_or(0) as usize;
    if count == 0 || count > 4096 || 132 + count * 12 > profile.len() {
        return true;
    }
    let mut entries = Vec::new();
    for index in 0..count {
        let base = 132 + index * 12;
        let signature = profile.get(base..base + 4).unwrap_or_default();
        let offset = be32(base + 4).unwrap_or(0) as usize;
        let size = be32(base + 8).unwrap_or(0) as usize;
        if offset
            .checked_add(size)
            .is_some_and(|end| end <= profile.len())
            && size > 0
        {
            entries.push((signature, offset, size));
        }
    }
    let find = |signature: &[u8]| {
        entries
            .iter()
            .find(|(name, _, _)| *name == signature)
            .map(|(_, offset, size)| (*offset, *size))
    };

    for (signature, name, group) in [
        (b"desc", "ProfileDescription", "ICC_Profile"),
        (b"cprt", "ProfileCopyright", "ICC_Profile"),
        (b"dmnd", "DeviceMfgDesc", "ICC_Profile"),
        (b"dmdd", "DeviceModelDesc", "ICC_Profile"),
        (b"vued", "ViewingCondDesc", "ICC_Profile"),
    ] {
        if let Some((offset, size)) = find(signature)
            && let Some(text) = text_tag(profile, offset, size)
        {
            add(out, group, name, Value::Str(text));
        }
    }
    for (signature, name) in [
        (b"wtpt", "MediaWhitePoint"),
        (b"bkpt", "MediaBlackPoint"),
        (b"rXYZ", "RedMatrixColumn"),
        (b"gXYZ", "GreenMatrixColumn"),
        (b"bXYZ", "BlueMatrixColumn"),
        (b"lumi", "Luminance"),
    ] {
        if let Some((offset, _)) = find(signature) {
            let value = xyz(profile, offset + 8, 12);
            if !value.is_empty() {
                add(out, "ICC_Profile", name, Value::Str(value));
            }
        }
    }
    if let Some((offset, _)) = find(b"chad") {
        let mut values = Vec::new();
        for index in 0..9 {
            let at = offset + 8 + index * 4;
            let Some(raw) = profile.get(at..at + 4) else {
                break;
            };
            let value = i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64 / 65536.0;
            values.push(trim_float(value, 5));
        }
        if values.len() == 9 {
            add(
                out,
                "ICC_Profile",
                "ChromaticAdaptation",
                Value::Str(values.join(" ")),
            );
        }
    }
    if let Some((offset, size)) = find(b"tech") {
        if let Some(signature) = profile.get(offset + 8..offset + 12) {
            add(
                out,
                "ICC_Profile",
                "Technology",
                Value::Str(technology(signature)),
            );
        }
        let _ = size;
    }
    for (signature, name) in [
        (b"rTRC", "RedTRC"),
        (b"gTRC", "GreenTRC"),
        (b"bTRC", "BlueTRC"),
    ] {
        if let Some((offset, size)) = find(signature) {
            add(
                out,
                "ICC_Profile",
                name,
                Value::Bytes(profile[offset..offset + size].to_vec()),
            );
        }
    }
    if let Some((offset, _)) = find(b"view") {
        add(
            out,
            "ICC-view",
            "ViewingCondIlluminant",
            Value::Str(xyz(profile, offset + 8, 12)),
        );
        add(
            out,
            "ICC-view",
            "ViewingCondSurround",
            Value::Str(xyz(profile, offset + 20, 12)),
        );
        let kind = be32(offset + 32).unwrap_or(0);
        add(
            out,
            "ICC-view",
            "ViewingCondIlluminantType",
            Value::Str(illuminant(kind)),
        );
    }
    if let Some((offset, _)) = find(b"meas") {
        let observer = match be32(offset + 8).unwrap_or(0) {
            1 => "CIE 1931",
            2 => "CIE 1964",
            _ => "Unknown",
        };
        add(
            out,
            "ICC-meas",
            "MeasurementObserver",
            Value::Str(observer.to_string()),
        );
        add(
            out,
            "ICC-meas",
            "MeasurementBacking",
            Value::Str(xyz(profile, offset + 12, 12)),
        );
        let geometry = match be32(offset + 24).unwrap_or(0) {
            1 => "0/45 or 45/0",
            2 => "0/d or d/0",
            _ => "Unknown",
        };
        add(
            out,
            "ICC-meas",
            "MeasurementGeometry",
            Value::Str(geometry.to_string()),
        );
        let flare = be32(offset + 28).unwrap_or(0) as f64 / 65536.0 * 100.0;
        add(
            out,
            "ICC-meas",
            "MeasurementFlare",
            Value::Str(format!("{}%", trim_float(flare, 3))),
        );
        add(
            out,
            "ICC-meas",
            "MeasurementIlluminant",
            Value::Str(illuminant(be32(offset + 32).unwrap_or(0))),
        );
    }
    true
}

fn text_tag(profile: &[u8], offset: usize, size: usize) -> Option<String> {
    let tag = profile.get(offset..offset + size)?;
    match tag.get(0..4)? {
        b"desc" => {
            let count =
                u32::from_be_bytes([*tag.get(8)?, *tag.get(9)?, *tag.get(10)?, *tag.get(11)?])
                    as usize;
            let text = tag.get(12..12 + count)?;
            Some(
                String::from_utf8_lossy(text)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        }
        b"text" => {
            let text = tag.get(8..)?;
            let end = text
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(text.len());
            Some(String::from_utf8_lossy(&text[..end]).to_string())
        }
        b"mluc" => {
            let _count =
                u32::from_be_bytes([*tag.get(8)?, *tag.get(9)?, *tag.get(10)?, *tag.get(11)?])
                    as usize;
            let record_size =
                u32::from_be_bytes([*tag.get(12)?, *tag.get(13)?, *tag.get(14)?, *tag.get(15)?])
                    as usize;
            let base = 16;
            let _record_size = record_size.max(12);
            let length = u32::from_be_bytes([
                *tag.get(base + 4)?,
                *tag.get(base + 5)?,
                *tag.get(base + 6)?,
                *tag.get(base + 7)?,
            ]) as usize;
            let at = u32::from_be_bytes([
                *tag.get(base + 8)?,
                *tag.get(base + 9)?,
                *tag.get(base + 10)?,
                *tag.get(base + 11)?,
            ]) as usize;
            let raw = tag.get(at..at + length)?;
            let units: Vec<u16> = raw
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            Some(
                String::from_utf16_lossy(&units)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        }
        b"utf8" => {
            let text = tag.get(8..)?;
            let end = text
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(text.len());
            Some(String::from_utf8_lossy(&text[..end]).to_string())
        }
        _ => None,
    }
}

fn xyz(profile: &[u8], offset: usize, length: usize) -> String {
    if length < 12 {
        return String::new();
    }
    let mut values = Vec::new();
    for index in 0..3 {
        let at = offset + index * 4;
        let Some(raw) = profile.get(at..at + 4) else {
            return String::new();
        };
        let value = i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64 / 65536.0;
        values.push(trim_float(value, 5));
    }
    values.join(" ")
}

fn trim_float(value: f64, decimals: usize) -> String {
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

fn company(signature: &str) -> String {
    match signature {
        "Lino" => "Linotronic",
        "MSFT" => "Microsoft Corporation",
        "HP  " | "IEC " => "Hewlett-Packard",
        "APPL" | "appl" => "Apple Computer Inc.",
        "ADBE" => "Adobe Systems Inc.",
        "SGI " => "Silicon Graphics Inc.",
        "SUNW" => "Sun Microsystems",
        "lcms" => "Little CMS",
        "" => "",
        _ => return format!("Unknown ({signature})"),
    }
    .to_string()
}

fn profile_class(signature: &str) -> String {
    match signature {
        "mntr" => "Display Device Profile",
        "scnr" => "Input Device Profile",
        "prtr" => "Output Device Profile",
        "spac" => "Color Space Conversion Profile",
        "abst" => "Abstract Profile",
        "link" => "DeviceLink Profile",
        "nmcl" => "Named Color Profile",
        _ => signature,
    }
    .to_string()
}

fn technology(signature: &[u8]) -> String {
    match signature {
        b"CRT " => "Cathode Ray Tube Display",
        b"LCD " => "Liquid Crystal Display",
        b"LED " => "Light Emitting Diode Display",
        b"PRNT" => "Printer",
        b"SCNR" => "Scanner",
        b"DCAM" => "Digital Camera",
        b"VIDM" => "Video Monitor",
        b"VIDC" => "Video Camera",
        b"AMBD" => "Ambient Light",
        _ => "Unknown",
    }
    .to_string()
}

fn illuminant(value: u32) -> String {
    match value {
        1 => "D50",
        2 => "D65",
        3 => "D93",
        4 => "F2",
        5 => "D55",
        6 => "A",
        7 => "Equi-Power (E)",
        8 => "F8",
        _ => "Unknown",
    }
    .to_string()
}
