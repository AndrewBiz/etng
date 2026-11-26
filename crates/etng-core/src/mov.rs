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
            .chunks_exact(4)
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
}
