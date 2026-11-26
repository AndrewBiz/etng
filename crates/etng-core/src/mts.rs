//! M2TS/MPEG-TS demux for Sony AVCHD metadata.
//!
//! We only need the video PID payload: H.264 SEI user data carries the MDPM
//! (Modified Digital Video Pack Metadata) with date/time and GPS.

use std::collections::HashMap;

use crate::model::{Tag, Value};

const MDPM_SIGNATURE: [u8; 20] = [
    0x17, 0xee, 0x8c, 0x60, 0xf8, 0x4d, 0x11, 0xd9, 0x8c, 0xd6, 0x08, 0x00, 0x20, 0x0c, 0x9a, 0x66,
    b'M', b'D', b'P', b'M',
];

pub fn parse(data: &[u8], out: &mut Vec<Tag>) -> bool {
    let step = if data.get(4) == Some(&0x47) && (data.len() <= 196 || data.get(196) == Some(&0x47))
    {
        192usize
    } else if data.first() == Some(&0x47) && data.get(188) == Some(&0x47) {
        188usize
    } else {
        return false;
    };
    let start = if step == 192 { 4 } else { 0 };
    let mut streams: HashMap<u16, Vec<u8>> = HashMap::new();
    let mut offset = start;
    while offset + 188 <= data.len() {
        if data[offset] != 0x47 {
            offset += 1;
            continue;
        }
        let b1 = data[offset + 1];
        let b2 = data[offset + 2];
        let b3 = data[offset + 3];
        let pid = (((b1 & 0x1f) as u16) << 8) | b2 as u16;
        let adaptation = (b3 >> 4) & 0x3;
        let mut pos = offset + 4;
        if adaptation & 0x2 != 0 {
            let length = data[pos] as usize;
            pos += 1 + length;
        }
        if adaptation & 0x1 != 0 && pos < offset + 188 {
            let payload = &data[pos..offset + 188];
            let pusi = b1 & 0x40 != 0;
            let start = if pusi && payload.len() >= 9 && payload.get(0..3) == Some(&[0, 0, 1]) {
                (9 + payload[8] as usize).min(payload.len())
            } else {
                0
            };
            streams
                .entry(pid)
                .or_default()
                .extend_from_slice(&payload[start..]);
        }
        offset += step;
    }
    for stream in streams.values() {
        let stream = unescape(stream);
        if let Some(index) = find(&stream, &MDPM_SIGNATURE) {
            mdpm(&stream[index + MDPM_SIGNATURE.len()..], out);
            return true;
        }
    }
    false
}

fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut index = 0usize;
    while index < data.len() {
        if data[index] == 0 && data.get(index + 1) == Some(&0) && data.get(index + 2) == Some(&3) {
            out.push(0);
            out.push(0);
            index += 3;
        } else {
            out.push(data[index]);
            index += 1;
        }
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn mdpm(data: &[u8], out: &mut Vec<Tag>) {
    let Some(count) = data.first().copied() else {
        return;
    };
    let mut entries: Vec<(u8, [u8; 4])> = Vec::new();
    let mut pos = 1usize;
    for _ in 0..count {
        let Some(tag) = data.get(pos).copied() else {
            break;
        };
        let Some(raw) = data.get(pos + 1..pos + 5) else {
            break;
        };
        entries.push((tag, [raw[0], raw[1], raw[2], raw[3]]));
        pos += 5;
    }
    let find = |tag: u8| {
        entries
            .iter()
            .find(|(id, _)| *id == tag)
            .map(|(_, raw)| *raw)
    };
    if let (Some(first), Some(second)) = (find(0x18), find(0x19)) {
        let mut clock = [0u8; 8];
        clock[..4].copy_from_slice(&first);
        clock[4..].copy_from_slice(&second);
        out.push(Tag::new(
            "H264",
            "H264",
            "DateTimeOriginal",
            Value::Str(date(&clock)),
        ));
    }
    if let Some(version) = find(0xb0) {
        out.push(Tag::new(
            "GPS",
            "GPS",
            "GPSVersionID",
            Value::Str(format!(
                "{}.{}.{}.{}",
                version[0], version[1], version[2], version[3]
            )),
        ));
    }
    if let Some(value) = find(0xb1) {
        out.push(
            Tag::new(
                "GPS",
                "GPS",
                "GPSLatitudeRef",
                Value::Str((value[0] as char).to_string()),
            )
            .with_print(if value[0] == b'S' { "South" } else { "North" }),
        );
    }
    if let Some(value) = rationals(&entries, 0xb2) {
        out.push(Tag::new(
            "GPS",
            "GPS",
            "GPSLatitude",
            Value::Str(dms(&value)),
        ));
    }
    if let Some(value) = find(0xb5) {
        out.push(
            Tag::new(
                "GPS",
                "GPS",
                "GPSLongitudeRef",
                Value::Str((value[0] as char).to_string()),
            )
            .with_print(if value[0] == b'W' { "West" } else { "East" }),
        );
    }
    if let Some(value) = rationals(&entries, 0xb6) {
        out.push(Tag::new(
            "GPS",
            "GPS",
            "GPSLongitude",
            Value::Str(dms(&value)),
        ));
    }
}

fn rationals(entries: &[(u8, [u8; 4])], first: u8) -> Option<[f64; 3]> {
    let mut values = Vec::new();
    for id in first..first + 3 {
        let (_, raw) = entries.iter().find(|(tag, _)| *tag == id)?;
        let num = u16::from_be_bytes([raw[0], raw[1]]) as f64;
        let den = u16::from_be_bytes([raw[2], raw[3]]) as f64;
        values.push(if den == 0.0 { 0.0 } else { num / den });
    }
    values.try_into().ok()
}

fn dms(values: &[f64; 3]) -> String {
    format!("{:.0} deg {:.0}' {:.2}\"", values[0], values[1], values[2])
}

fn date(clock: &[u8; 8]) -> String {
    let tz = clock[0];
    let hours = (tz >> 1) & 0x0f;
    let sign = if tz & 0x20 != 0 { '-' } else { '+' };
    let half = if tz & 0x01 != 0 { "30" } else { "00" };
    let dst = if tz & 0x40 != 0 { " DST" } else { "" };
    format!(
        "{:02x}{:02x}:{:02x}:{:02x} {:02x}:{:02x}:{:02x}{sign}{hours:02}:{half}{dst}",
        clock[1], clock[2], clock[3], clock[4], clock[5], clock[6], clock[7]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mdpm_date_and_gps() {
        let mut data = vec![0u8; 196];
        data[0..4].copy_from_slice(&[0, 0, 0, 0]);
        data[4] = 0x47;
        data[5] = 0x41; // payload unit start, PID 0x0111-ish
        data[6] = 0x11;
        data[7] = 0x10; // payload only
        let signature = MDPM_SIGNATURE;
        let mut payload = vec![0u8; 0];
        payload.extend_from_slice(&signature);
        payload.push(2);
        payload.extend_from_slice(&[0x18, 0x02, 0x20, 0x14, 0x01]);
        payload.extend_from_slice(&[0x19, 0x03, 0x13, 0x26, 0x26]);
        data[8..8 + payload.len()].copy_from_slice(&payload);
        let mut tags = Vec::new();
        assert!(parse(&data, &mut tags));
        assert_eq!(
            tags.iter()
                .find(|tag| tag.name == "DateTimeOriginal")
                .map(|tag| tag.value.display())
                .as_deref(),
            Some("2014:01:03 13:26:26+01:00")
        );
    }
}
