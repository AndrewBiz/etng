//! Minimal QuickTime/MOV metadata: ftyp, movie header and media data.

use crate::model::{Tag, Value};

const QT_EPOCH_OFFSET: i64 = 2_082_844_800;

struct Atom<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

fn atoms(data: &[u8]) -> Vec<Atom<'_>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = be32(&data[pos..]).unwrap_or(0);
        let kind = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let (header, size) = if size32 == 1 {
            let Some(raw) = data.get(pos + 8..pos + 16) else {
                break;
            };
            (16usize, be64(raw).unwrap_or(0) as usize)
        } else if size32 == 0 {
            (8usize, data.len().saturating_sub(pos))
        } else {
            (8usize, size32 as usize)
        };
        if size < header || pos + size > data.len() {
            break;
        }
        out.push(Atom {
            kind,
            payload: &data[pos + header..pos + size],
        });
        pos += size;
    }
    out
}

fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    atoms(data)
        .into_iter()
        .find(|atom| &atom.kind == kind)
        .map(|atom| atom.payload)
}

pub fn parse(data: &[u8], out: &mut Vec<Tag>) {
    let top = atoms(data);
    if let Some(ftyp) = top.iter().find(|atom| &atom.kind == b"ftyp") {
        let payload = ftyp.payload;
        if let Some(brand) = payload.get(0..4) {
            let label = match brand {
                b"qt  " => "Apple QuickTime (.MOV/QT)",
                b"isom" => "ISO Base Media",
                b"mp42" => "MP4 v2",
                _ => "Unknown",
            };
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MajorBrand",
                Value::Str(label.to_string()),
            ));
        }
        if let Some(minor) = be32(payload.get(4..).unwrap_or_default()) {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MinorVersion",
                Value::Str(format!(
                    "{}.{}.{}",
                    (minor >> 16) & 0xFF,
                    (minor >> 8) & 0xFF,
                    minor & 0xFF
                )),
            ));
        }
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

    if let Some(moov) = top.iter().find(|atom| &atom.kind == b"moov")
        && let Some(mvhd) = child(moov.payload, b"mvhd")
    {
        let version = mvhd.first().copied().unwrap_or(0);
        let (created, modified, scale, duration) = if version == 1 {
            (
                be64(mvhd.get(4..).unwrap_or_default()),
                be64(mvhd.get(12..).unwrap_or_default()),
                be32(mvhd.get(20..).unwrap_or_default()),
                be64(mvhd.get(24..).unwrap_or_default()),
            )
        } else {
            (
                be32(mvhd.get(4..).unwrap_or_default()).map(u64::from),
                be32(mvhd.get(8..).unwrap_or_default()).map(u64::from),
                be32(mvhd.get(12..).unwrap_or_default()),
                be32(mvhd.get(16..).unwrap_or_default()).map(u64::from),
            )
        };
        if let Some(created) = created {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "CreateDate",
                Value::Str(qt_date(created)),
            ));
        }
        if let Some(modified) = modified {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "ModifyDate",
                Value::Str(qt_date(modified)),
            ));
        }
        if let (Some(scale), Some(duration)) = (scale, duration)
            && scale > 0
        {
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "Duration",
                Value::Str(format!("{:.2} s", duration as f64 / scale as f64)),
            ));
        }
    }

    parse_apple_keys(&top, out);
    parse_content_create_date(&top, out);

    for atom in &top {
        if &atom.kind == b"mdat" {
            let total = atom.payload.len();
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MediaDataSize",
                Value::Int(total as i64),
            ));
            out.push(Tag::new(
                "QuickTime",
                "QuickTime",
                "MediaDataOffset",
                Value::Int((data.len() - total) as i64),
            ));
        }
    }
}

fn parse_apple_keys(top: &[Atom<'_>], out: &mut Vec<Tag>) {
    let Some(moov) = top.iter().find(|atom| &atom.kind == b"moov") else {
        return;
    };
    let Some(meta) = find_descendant(moov.payload, b"meta", 2) else {
        return;
    };
    let meta_payload = if atoms(meta).is_empty() && meta.len() > 4 {
        &meta[4..]
    } else {
        meta
    };
    let Some(keys) = child(meta_payload, b"keys") else {
        return;
    };
    let Some(ilst) = child(meta_payload, b"ilst") else {
        return;
    };
    let names = parse_key_names(keys);
    if names.is_empty() {
        return;
    }
    for item in atoms(ilst) {
        let index = u32::from_be_bytes(item.kind) as usize;
        let Some(key) = names.get(index.saturating_sub(1)).map(String::as_str) else {
            continue;
        };
        let Some(data) = child(item.payload, b"data") else {
            continue;
        };
        if data.len() < 8 || be32(&data[..4]) != Some(1) {
            continue;
        }
        let value = String::from_utf8_lossy(&data[8..])
            .trim_end_matches('\0')
            .to_string();
        let (name, value) = match key {
            "com.apple.quicktime.creationdate" => ("CreationDate", apple_date(&value)),
            "com.apple.quicktime.make" => ("Make", value),
            "com.apple.quicktime.model" => ("Model", value),
            "com.apple.quicktime.software" => ("Software", value),
            "com.apple.quicktime.location.ISO6709" => ("GPSCoordinates", apple_gps(&value)),
            "com.apple.quicktime.content.identifier" => ("ContentIdentifier", value),
            _ => continue,
        };
        if !value.is_empty() {
            out.push(Tag::new("QuickTime", "Keys", name, Value::Str(value)));
        }
    }
}

fn find_descendant<'a>(data: &'a [u8], kind: &[u8; 4], depth: u8) -> Option<&'a [u8]> {
    for atom in atoms(data) {
        if &atom.kind == kind {
            return Some(atom.payload);
        }
        if depth > 0
            && matches!(&atom.kind, b"moov" | b"udta" | b"meta")
            && let Some(found) = find_descendant(atom.payload, kind, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn parse_key_names(payload: &[u8]) -> Vec<String> {
    let Some(count) = be32(payload.get(4..).unwrap_or_default()) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut pos = 8usize;
    for _ in 0..count {
        let Some(size) = be32(payload.get(pos..).unwrap_or_default()) else {
            break;
        };
        let size = size as usize;
        let end = pos.saturating_add(size);
        if size < 8 || end > payload.len() {
            break;
        }
        let name = String::from_utf8_lossy(&payload[pos + 8..end])
            .trim_end_matches('\0')
            .to_string();
        names.push(name);
        pos = end;
    }
    names
}

fn parse_content_create_date(top: &[Atom<'_>], out: &mut Vec<Tag>) {
    let Some(moov) = top.iter().find(|atom| &atom.kind == b"moov") else {
        return;
    };
    let Some(udta) = child(moov.payload, b"udta") else {
        return;
    };
    for atom in atoms(udta) {
        if &atom.kind == b"\xa9day"
            && let Some(value) = text_value(atom.payload)
        {
            out.push(Tag::new(
                "QuickTime",
                "UserData",
                "ContentCreateDate",
                Value::Str(apple_date(&value)),
            ));
        }
        if &atom.kind == b"meta" {
            let meta = if atoms(atom.payload).is_empty() && atom.payload.len() > 4 {
                &atom.payload[4..]
            } else {
                atom.payload
            };
            let Some(ilst) = child(meta, b"ilst") else {
                continue;
            };
            for item in atoms(ilst) {
                if &item.kind != b"\xa9day" {
                    continue;
                }
                let Some(data) = child(item.payload, b"data") else {
                    continue;
                };
                if data.len() < 8 || be32(&data[..4]) != Some(1) {
                    continue;
                }
                let value = String::from_utf8_lossy(&data[8..])
                    .trim_end_matches('\0')
                    .to_string();
                if !value.is_empty() {
                    out.push(Tag::new(
                        "QuickTime",
                        "ItemList",
                        "ContentCreateDate",
                        Value::Str(apple_date(&value)),
                    ));
                }
            }
        }
    }
}

fn text_value(data: &[u8]) -> Option<String> {
    let value = String::from_utf8_lossy(data)
        .trim_matches(|ch: char| ch == '\0' || ch.is_whitespace())
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

fn apple_gps(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches('/');
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (index, byte) in raw.bytes().enumerate() {
        if index > 0 && (byte == b'+' || byte == b'-') {
            parts.push(&raw[start..index]);
            start = index;
        }
    }
    if start < raw.len() {
        parts.push(&raw[start..]);
    }
    let Some(latitude) = parts.first().and_then(|part| part.parse::<f64>().ok()) else {
        return raw.to_string();
    };
    let Some(longitude) = parts.get(1).and_then(|part| part.parse::<f64>().ok()) else {
        return raw.to_string();
    };
    let mut out = format!(
        "{}, {}",
        apple_coordinate(latitude, 'N', 'S'),
        apple_coordinate(longitude, 'E', 'W')
    );
    if let Some(altitude) = parts.get(2).and_then(|part| part.parse::<f64>().ok()) {
        let direction = if altitude < 0.0 {
            "Below Sea Level"
        } else {
            "Above Sea Level"
        };
        out.push_str(&format!(", {:.3} m {direction}", altitude.abs()));
    }
    out
}

fn apple_coordinate(value: f64, positive: char, negative: char) -> String {
    let hemisphere = if value < 0.0 { negative } else { positive };
    let mut degrees = value.abs().floor() as i64;
    let minutes_total = (value.abs() - degrees as f64) * 60.0;
    let mut minutes = minutes_total.floor() as i64;
    let mut seconds = (minutes_total - minutes as f64) * 60.0;
    if seconds >= 59.995 {
        seconds = 0.0;
        minutes += 1;
    }
    if minutes >= 60 {
        minutes = 0;
        degrees += 1;
    }
    format!("{degrees} deg {minutes}' {seconds:.2}\" {hemisphere}")
}

fn apple_date(raw: &str) -> String {
    let raw = raw.trim();
    let Some((date, time)) = raw.split_once('T') else {
        return raw.replace('-', ":");
    };
    let date = date.replace('-', ":");
    let (time, zone) = match time.rfind(['+', '-']) {
        Some(index) if time.len() - index == 5 => (&time[..index], Some(&time[index..])),
        _ => (time, None),
    };
    let time = time.split('.').next().unwrap_or(time);
    match zone {
        Some(zone) => {
            let sign = &zone[..1];
            let digits = &zone[1..];
            format!("{date} {time}{sign}{}:{}", &digits[..2], &digits[2..])
        }
        None => format!("{date} {time}"),
    }
}

fn qt_date(seconds: u64) -> String {
    chrono::DateTime::from_timestamp(seconds as i64 - QT_EPOCH_OFFSET, 0)
        .map(|value| value.format("%Y:%m:%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

fn be32(bytes: &[u8]) -> Option<u32> {
    let raw = bytes.get(0..4)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn be64(bytes: &[u8]) -> Option<u64> {
    let raw = bytes.get(0..8)?;
    Some(u64::from_be_bytes([
        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn reads_movie_header_dates() {
        let mut ftyp = Vec::new();
        ftyp.extend_from_slice(b"qt  ");
        ftyp.extend_from_slice(&0u32.to_be_bytes());
        ftyp.extend_from_slice(b"qt  ");

        let created = 1483668246u32 + QT_EPOCH_OFFSET as u32;
        let mut mvhd = vec![0u8; 100];
        mvhd[4..8].copy_from_slice(&created.to_be_bytes());
        mvhd[8..12].copy_from_slice(&created.to_be_bytes());
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&2970u32.to_be_bytes());
        let moov = atom(b"moov", &atom(b"mvhd", &mvhd));

        let mut data = atom(b"ftyp", &ftyp);
        data.extend_from_slice(&moov);
        data.extend_from_slice(&atom(b"mdat", &[0; 4]));

        let mut tags = Vec::new();
        parse(&data, &mut tags);
        let get = |name: &str| {
            tags.iter()
                .find(|tag| tag.name == name)
                .map(|tag| tag.value.display())
        };
        assert_eq!(
            get("MajorBrand").as_deref(),
            Some("Apple QuickTime (.MOV/QT)")
        );
        assert_eq!(get("CreateDate").as_deref(), Some("2017:01:06 02:04:06"));
        assert_eq!(get("Duration").as_deref(), Some("2.97 s"));
        assert_eq!(get("MediaDataSize").as_deref(), Some("4"));
    }

    #[test]
    fn reads_apple_keys_creation_date() {
        fn key(name: &str) -> Vec<u8> {
            let mut out = Vec::new();
            let size = 8 + name.len();
            out.extend_from_slice(&(size as u32).to_be_bytes());
            out.extend_from_slice(b"mdta");
            out.extend_from_slice(name.as_bytes());
            out
        }
        fn data(value: &str) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&1u32.to_be_bytes());
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(value.as_bytes());
            atom(b"data", &out)
        }

        let keys = [
            "com.apple.quicktime.make",
            "com.apple.quicktime.model",
            "com.apple.quicktime.software",
            "com.apple.quicktime.creationdate",
            "com.apple.quicktime.location.ISO6709",
            "com.apple.quicktime.content.identifier",
        ];
        let mut keys_payload = vec![0u8; 4];
        keys_payload.extend_from_slice(&(keys.len() as u32).to_be_bytes());
        for name in keys {
            keys_payload.extend_from_slice(&key(name));
        }
        let mut ilst_payload = Vec::new();
        for (index, value) in [
            "Apple",
            "iPhone 6s Plus",
            "14.4",
            "2021-02-22T17:28:32+0300",
            "+55.0132+037.4721+184.497/",
            "9B14D7B9-6A47-443E-A565-E2E7ECE6A604",
        ]
        .iter()
        .enumerate()
        {
            ilst_payload
                .extend_from_slice(&atom(&((index + 1) as u32).to_be_bytes(), &data(value)));
        }
        let meta = atom(
            b"meta",
            &[
                atom(b"hdlr", &[0; 24]),
                atom(b"keys", &keys_payload),
                atom(b"ilst", &ilst_payload),
            ]
            .concat(),
        );
        let udta = atom(b"udta", &atom(b"\xa9day", b"2017-01-06T12:04:06+1000"));
        let data = atom(b"moov", &[meta, udta].concat());

        let mut tags = Vec::new();
        parse(&data, &mut tags);
        let get = |name: &str| {
            tags.iter()
                .find(|tag| tag.name == name)
                .map(|tag| (tag.group1, tag.value.display()))
        };
        assert_eq!(
            get("CreationDate"),
            Some(("Keys", "2021:02:22 17:28:32+03:00".to_string()))
        );
        assert_eq!(get("Make"), Some(("Keys", "Apple".to_string())));
        assert_eq!(get("Model"), Some(("Keys", "iPhone 6s Plus".to_string())));
        assert_eq!(get("Software"), Some(("Keys", "14.4".to_string())));
        assert_eq!(
            get("GPSCoordinates"),
            Some((
                "Keys",
                "55 deg 0' 47.52\" N, 37 deg 28' 19.56\" E, 184.497 m Above Sea Level".to_string()
            ))
        );
        assert_eq!(
            get("ContentIdentifier"),
            Some(("Keys", "9B14D7B9-6A47-443E-A565-E2E7ECE6A604".to_string()))
        );
        assert_eq!(
            get("ContentCreateDate"),
            Some(("UserData", "2017:01:06 12:04:06+10:00".to_string()))
        );
    }
}
