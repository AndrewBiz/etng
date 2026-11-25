//! Canon CR3 (ISO-BMFF) reader: Canon CMT metadata, previews and QuickTime.

use etng_tags::tables;

use crate::formatting::{format_float, format_unix};
use crate::model::{Tag, Value};
use crate::options::ReadOptions;
use crate::tiff::{Entry, Tiff};
use crate::{exif, makernotes, xmp};

const QT_EPOCH_OFFSET: i64 = 2_082_844_800; // 1904-01-01 .. 1970-01-01

struct BoxRef<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

fn boxes(data: &[u8]) -> Vec<BoxRef<'_>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size32 = be32(&data[pos..]);
        let kind = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let (header, size) = if size32 == 1 {
            let Some(raw) = data.get(pos + 8..pos + 16) else {
                break;
            };
            (16usize, be64(raw) as usize)
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

fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<BoxRef<'a>> {
    boxes(data).into_iter().find(|b| &b.kind == kind)
}

/// Canon boxes live inside a `uuid` payload without a standard tree; find by
/// kind signature and validate the size field immediately before it.
fn find_kind<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let mut pos = 4usize;
    while pos + 4 <= data.len() {
        if &data[pos..pos + 4] == kind {
            let size = be32(&data[pos - 4..]) as usize;
            if size >= 8 && pos - 4 + size <= data.len() {
                return Some(&data[pos + 4..pos - 4 + size]);
            }
        }
        pos += 1;
    }
    None
}

fn be16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn be64(bytes: &[u8]) -> u64 {
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn at16(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2).map(be16)
}

fn at32(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4).map(be32)
}

fn qt_date(seconds: u32) -> String {
    format_unix(seconds as i64 - QT_EPOCH_OFFSET, 0)
}

fn seconds(duration: u32, timescale: u32) -> String {
    if timescale == 0 {
        return "0 s".to_string();
    }
    format!("{:.2} s", duration as f64 / timescale as f64)
}

fn matrix(payload: &[u8]) -> String {
    let mut values = Vec::with_capacity(9);
    for index in 0..9 {
        let Some(raw) = payload.get(index * 4..index * 4 + 4) else {
            return String::new();
        };
        let value = be32(raw) as i32 as f64 / 65536.0;
        let value = if index == 8 { value / 16384.0 } else { value };
        values.push(format_float(value));
    }
    values.join(" ")
}

pub(crate) fn parse(data: &[u8], opts: &ReadOptions) -> (Vec<Tag>, Vec<String>, Option<String>) {
    let mut tags = Vec::new();
    let mut warnings = Vec::new();
    let Some(ftyp) = child(data, b"ftyp") else {
        return (tags, warnings, Some("CR3: missing ftyp box".to_string()));
    };

    quicktime_brand(ftyp.payload, &mut tags);
    if let Some(mvhd) = child(data, b"moov").and_then(|moov| child(moov.payload, b"mvhd")) {
        movie_header(mvhd.payload, &mut tags);
    }
    if let Some(preview) = preview_image(data) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "PreviewImage",
            Value::Bytes(preview.to_vec()),
        ));
    }
    if let Some((size, offset)) = media_data(data) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MediaDataSize",
            Value::Int(size),
        ));
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MediaDataOffset",
            Value::Int(offset),
        ));
    }

    let little_endian = data
        .windows(4)
        .find(|w| *w == b"II*\0")
        .map(|_| true)
        .unwrap_or(false);

    if let Some(cmt3) = find_kind(data, b"CMT3") {
        makernotes::parse(
            Some("Canon"),
            cmt3,
            little_endian,
            &mut tags,
            &mut warnings,
            opts.numeric,
        );
        canon_subtables(cmt3, &mut tags, opts.numeric);
    }
    if let Some(payload) = find_kind(data, b"THMB") {
        thumbnail(payload, &mut tags);
    }
    if let Some(cmt1) = find_kind(data, b"CMT1") {
        exif::parse(cmt1, little_endian, &mut tags, &mut warnings, opts.numeric);
    }
    if let Some(cmt2) = find_kind(data, b"CMT2") {
        let mut exif_tags = Vec::new();
        exif::parse(
            cmt2,
            little_endian,
            &mut exif_tags,
            &mut warnings,
            opts.numeric,
        );
        for mut tag in exif_tags {
            if tag.group1 == "IFD0" {
                tag.group1 = "ExifIFD";
            }
            tags.push(tag);
        }
    }
    if let Some(cmt4) = find_kind(data, b"CMT4") {
        gps_block(cmt4, little_endian, &mut tags, opts.numeric);
    }

    tracks(data, &mut tags);

    for packet in xmp_packets(data) {
        xmp::parse(packet, &mut tags, &mut warnings);
    }

    let width = find_int(&tags, "ImageWidth");
    let height = find_int(&tags, "ImageHeight");
    crate::read::push_aperture(&mut tags);
    crate::read::push_composite(&mut tags, width, height);
    crate::read::push_post_composites(&mut tags);

    (tags, warnings, None)
}

fn quicktime_brand(payload: &[u8], tags: &mut Vec<Tag>) {
    let major = payload.get(0..4).unwrap_or_default();
    let label = match major {
        b"crx " => "Canon Raw (.CRX)",
        b"isom" => "ISO Base Media",
        _ => "Unknown",
    };
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "MajorBrand",
        Value::Str(label.to_string()),
    ));
    if let Some(minor) = at32(payload, 4) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MinorVersion",
            Value::Str(format!("0.0.{minor}")),
        ));
    }
    let mut brands = Vec::new();
    for chunk in payload.get(8..).unwrap_or_default().chunks(4) {
        if chunk.len() == 4 {
            brands.push(String::from_utf8_lossy(chunk).trim_end().to_string());
        }
    }
    if !brands.is_empty() {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "CompatibleBrands",
            Value::Str(brands.join(", ")),
        ));
    }
}

fn movie_header(payload: &[u8], tags: &mut Vec<Tag>) {
    let version = payload.first().copied().unwrap_or(0);
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "MovieHeaderVersion",
        Value::Int(version as i64),
    ));
    let base = if version == 1 { 16 } else { 8 };
    let (Some(created), Some(modified), Some(timescale), Some(duration)) = (
        at32(payload, base - 4),
        at32(payload, base),
        at32(payload, base + 4),
        at32(payload, base + 8),
    ) else {
        return;
    };
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "CreateDate",
        Value::Str(qt_date(created)),
    ));
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "ModifyDate",
        Value::Str(qt_date(modified)),
    ));
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "TimeScale",
        Value::Int(timescale as i64),
    ));
    tags.push(Tag::new(
        "QuickTime",
        "QuickTime",
        "Duration",
        Value::Str(seconds(duration, timescale)),
    ));

    let rate_offset = base + 12;
    if let Some(rate) = at32(payload, rate_offset) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "PreferredRate",
            Value::Float(rate as f64 / 65536.0),
        ));
    }
    if let Some(volume) = at16(payload, rate_offset + 4) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "PreferredVolume",
            Value::Str(format!("{:.2}%", volume as f64 / 256.0 * 100.0)),
        ));
    }
    let matrix_offset = 36;
    if let Some(matrix_bytes) = payload.get(matrix_offset..matrix_offset + 36) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "MatrixStructure",
            Value::Str(matrix(matrix_bytes)),
        ));
    }
    let preview_offset = 72;
    for (name, offset) in [
        ("PreviewTime", 0),
        ("PreviewDuration", 4),
        ("PosterTime", 8),
        ("SelectionTime", 12),
        ("SelectionDuration", 16),
        ("CurrentTime", 20),
    ] {
        if let Some(value) = at32(payload, preview_offset + offset) {
            tags.push(Tag::new(
                "QuickTime",
                "QuickTime",
                name,
                Value::Str(format!("{value} s")),
            ));
        }
    }
    if let Some(next) = at32(payload, preview_offset + 24) {
        tags.push(Tag::new(
            "QuickTime",
            "QuickTime",
            "NextTrackID",
            Value::Int(next as i64),
        ));
    }
}

fn canon_subtables(note: &[u8], tags: &mut Vec<Tag>, numeric: bool) {
    const TABLES: [(u16, &str); 8] = [
        (0x01, "Canon::CameraSettings"),
        (0x02, "Canon::FocalLength"),
        (0x04, "Canon::ShotInfo"),
        (0x93, "Canon::FileInfo"),
        (0x98, "Canon::CropInfo"),
        (0x9a, "Canon::AspectInfo"),
        (0xa0, "Canon::Processing"),
        (0xe0, "Canon::SensorInfo"),
    ];
    let Some(tiff) = Tiff::new(note) else {
        return;
    };
    let Some(offset) = tiff.first_ifd_offset() else {
        return;
    };
    let Some(ifd) = tiff.read_ifd(offset) else {
        return;
    };
    for entry in &ifd.entries {
        let Some((_, table)) = TABLES.iter().find(|(tag, _)| *tag == entry.tag) else {
            continue;
        };
        let Some(bytes) = tiff.entry_bytes(entry) else {
            continue;
        };
        let width = match entry.typ {
            3 | 8 => 2usize,
            4 | 9 => 4usize,
            _ => continue,
        };
        let defs = tables().defs_in_table("Canon", table);
        for def in defs {
            let Ok(index) = def.id.parse::<usize>() else {
                continue;
            };
            let start = index.saturating_mul(width);
            let Some(chunk) = bytes.get(start..start + width) else {
                continue;
            };
            let value = if width == 2 {
                Value::Int(if tiff.le {
                    i16::from_le_bytes([chunk[0], chunk[1]]) as i64
                } else {
                    i16::from_be_bytes([chunk[0], chunk[1]]) as i64
                })
            } else {
                Value::Int(if tiff.le {
                    i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i64
                } else {
                    be32(chunk) as i32 as i64
                })
            };
            let mut tag = Tag::new("MakerNotes", "Canon", def.name, value);
            if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
                tag = tag.with_print(print);
            }
            tags.push(tag);
        }
    }
}

fn thumbnail(payload: &[u8], tags: &mut Vec<Tag>) {
    let Some(length) = at32(payload, 8) else {
        return;
    };
    let Some(bytes) = payload.get(16..16 + length as usize) else {
        return;
    };
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return;
    }
    tags.push(Tag::new(
        "MakerNotes",
        "Canon",
        "ThumbnailImage",
        Value::Bytes(bytes.to_vec()),
    ));
}

fn preview_image(data: &[u8]) -> Option<&[u8]> {
    let payload = find_kind(data, b"PRVW")?;
    let bytes = payload.get(16..)?;
    bytes.starts_with(&[0xFF, 0xD8]).then_some(bytes)
}

fn xmp_packets(data: &[u8]) -> Vec<&[u8]> {
    let mut packets = Vec::new();
    for uuid in boxes(data).into_iter().filter(|b| &b.kind == b"uuid") {
        if let Some(pos) = uuid
            .payload
            .windows(9)
            .position(|window| window == b"<?xpacket")
        {
            packets.push(&uuid.payload[pos..]);
        }
    }
    packets
}

fn gps_block(payload: &[u8], little_endian: bool, tags: &mut Vec<Tag>, numeric: bool) {
    let Some(tiff) = Tiff::new(payload) else {
        return;
    };
    let _le = tiff.le || little_endian;
    let Some(offset) = tiff.first_ifd_offset() else {
        return;
    };
    let Some(ifd) = tiff.read_ifd(offset) else {
        return;
    };
    for entry in &ifd.entries {
        let id = entry.tag.to_string();
        let Some(def) = tables().find_table_id("GPS::Main", &id) else {
            continue;
        };
        let Some(value) = gps_value(&tiff, entry) else {
            continue;
        };
        let value = if def.name == "GPSVersionID" {
            gps_version(&value)
        } else if def.name == "GPSTimeStamp" {
            gps_time(&tiff, entry).unwrap_or(value)
        } else if matches!(def.name, "GPSLatitude" | "GPSLongitude") {
            match gps_decimal(&tiff, entry) {
                Some(decimal) => Value::Float(decimal),
                None => value,
            }
        } else {
            value
        };
        let mut tag = Tag::new("EXIF", "GPS", def.name, value);
        if !numeric {
            if let Some(print) = exif::special_print(def.name, &tag.value) {
                tag = tag.with_print(print);
            } else if let Some(print) = def.print_value(&tag.value.display()) {
                tag = tag.with_print(print);
            }
        }
        tags.push(tag);
    }
    gps_coordinate_prints(tags);
}

fn gps_version(value: &Value) -> Value {
    let text = value.display();
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() == 4 {
        Value::Str(parts.join("."))
    } else {
        value.clone()
    }
}

fn gps_time(tiff: &Tiff<'_>, entry: &Entry) -> Option<Value> {
    if entry.count < 3 || entry.typ != 5 {
        return None;
    }
    let bytes = tiff.entry_bytes(entry)?;
    let mut parts = [0u32; 3];
    for (index, part) in parts.iter_mut().enumerate() {
        let chunk = bytes.get(index * 8..index * 8 + 8)?;
        let num = if tiff.le {
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        } else {
            be32(chunk)
        };
        let den = if tiff.le {
            u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
        } else {
            be32(&chunk[4..])
        };
        if den == 0 {
            return None;
        }
        *part = (num as f64 / den as f64).round() as u32;
    }
    Some(Value::Str(format!(
        "{:02}:{:02}:{:02}",
        parts[0], parts[1], parts[2]
    )))
}

fn gps_coordinate_prints(tags: &mut [Tag]) {
    for tag in tags.iter_mut() {
        if !matches!(tag.name, "GPSLatitude" | "GPSLongitude") {
            continue;
        }
        let Some(decimal) = tag.value.as_f64() else {
            continue;
        };
        if let Some(print) = exif::format_coordinate(decimal, None) {
            tag.print = Some(print);
        }
    }
}

fn gps_decimal(tiff: &Tiff<'_>, entry: &Entry) -> Option<f64> {
    if entry.count < 3 || entry.typ != 5 {
        return None;
    }
    let bytes = tiff.entry_bytes(entry)?;
    let mut parts = [0f64; 3];
    for (index, part) in parts.iter_mut().enumerate() {
        let chunk = bytes.get(index * 8..index * 8 + 8)?;
        let num = if tiff.le {
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        } else {
            be32(chunk)
        };
        let den = if tiff.le {
            u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
        } else {
            be32(&chunk[4..])
        };
        if den == 0 {
            return None;
        }
        *part = num as f64 / den as f64;
    }
    Some(parts[0] + parts[1] / 60.0 + parts[2] / 3600.0)
}

fn gps_value(tiff: &Tiff<'_>, entry: &Entry) -> Option<Value> {
    let bytes = tiff.entry_bytes(entry)?;
    match entry.typ {
        2 => Some(Value::Str(
            String::from_utf8_lossy(bytes)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
        )),
        1 | 6 | 7 => int_value(bytes, entry.count as usize, 1, tiff.le),
        3 | 8 => int_value(bytes, entry.count as usize, 2, tiff.le),
        4 | 9 => int_value(bytes, entry.count as usize, 4, tiff.le),
        5 | 10 => {
            let chunk = bytes.get(0..8)?;
            let num = if tiff.le {
                u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
            } else {
                be32(chunk)
            };
            let den = if tiff.le {
                u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
            } else {
                be32(&chunk[4..])
            };
            Some(Value::Rational {
                num: num as i64,
                den: den as i64,
            })
        }
        _ => None,
    }
}

fn int_value(bytes: &[u8], count: usize, width: usize, le: bool) -> Option<Value> {
    let count = count.min(bytes.len() / width).max(1);
    let read = |chunk: &[u8]| -> i64 {
        match width {
            1 => chunk[0] as i64,
            2 => {
                if le {
                    u16::from_le_bytes([chunk[0], chunk[1]]) as i64
                } else {
                    be16(chunk) as i64
                }
            }
            _ => {
                if le {
                    u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i64
                } else {
                    be32(chunk) as i64
                }
            }
        }
    };
    if count == 1 {
        return Some(Value::Int(read(bytes)));
    }
    Some(Value::Str(
        (0..count)
            .map(|index| read(&bytes[index * width..]).to_string())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn find_int(tags: &[Tag], name: &str) -> Option<u32> {
    tags.iter()
        .find(|tag| tag.name == name)
        .and_then(|tag| tag.value.as_i64())
        .and_then(|value| u32::try_from(value).ok())
}

fn media_data(data: &[u8]) -> Option<(i64, i64)> {
    let mut pos = 4usize;
    while pos + 4 <= data.len() {
        if &data[pos..pos + 4] == b"mdat" {
            let declared = be32(&data[pos - 4..]) as u64;
            let (header, size) = if declared == 1 {
                let raw = data.get(pos + 4..pos + 12)?;
                (16u64, be64(raw))
            } else {
                (8u64, declared)
            };
            return Some(((size - header) as i64, (pos - 4) as i64 + header as i64));
        }
        pos += 1;
    }
    None
}

const TRACK_GROUPS: [&str; 4] = ["Track1", "Track2", "Track3", "Track4"];

fn tracks(data: &[u8], tags: &mut Vec<Tag>) {
    let Some(moov) = child(data, b"moov") else {
        return;
    };
    for (index, trak) in boxes(moov.payload)
        .into_iter()
        .filter(|b| &b.kind == b"trak")
        .enumerate()
    {
        let Some(group) = TRACK_GROUPS.get(index) else {
            break;
        };
        track(trak.payload, group, data, tags);
    }
}

fn track(payload: &[u8], group: &'static str, data: &[u8], tags: &mut Vec<Tag>) {
    let mut push = |name: &'static str, value: Value, print: Option<String>| {
        let mut tag = Tag::new("QuickTime", group, name, value);
        if let Some(print) = print {
            tag = tag.with_print(print);
        }
        tags.push(tag);
    };

    if let Some(tkhd) = child(payload, b"tkhd") {
        let p = tkhd.payload;
        let version = p.first().copied().unwrap_or(0);
        push("TrackHeaderVersion", Value::Int(version as i64), None);
        if version == 0 {
            if let (Some(created), Some(modified), Some(id), Some(duration)) =
                (at32(p, 4), at32(p, 8), at32(p, 12), at32(p, 20))
            {
                push("TrackCreateDate", Value::Str(qt_date(created)), None);
                push("TrackModifyDate", Value::Str(qt_date(modified)), None);
                push("TrackID", Value::Int(id as i64), None);
                push(
                    "TrackDuration",
                    Value::Str(seconds(duration, movie_timescale(data))),
                    None,
                );
            }
            if let Some(layer) = at16(p, 32) {
                push("TrackLayer", Value::Int(layer as i64), None);
            }
            if let Some(volume) = at16(p, 36) {
                push(
                    "TrackVolume",
                    Value::Str(format!("{:.2}%", volume as f64 / 256.0 * 100.0)),
                    None,
                );
            }
            if let Some(matrix_bytes) = p.get(40..76) {
                push("MatrixStructure", Value::Str(matrix(matrix_bytes)), None);
            }
            if let Some(width) = at32(p, 76) {
                push(
                    "ImageWidth",
                    Value::Int((width as f64 / 65536.0).round() as i64),
                    None,
                );
            }
            if let Some(height) = at32(p, 80) {
                push(
                    "ImageHeight",
                    Value::Int((height as f64 / 65536.0).round() as i64),
                    None,
                );
            }
        }
    }

    if let Some(mdia) = child(payload, b"mdia") {
        if let Some(mdhd) = child(mdia.payload, b"mdhd") {
            let p = mdhd.payload;
            let version = p.first().copied().unwrap_or(0);
            push("MediaHeaderVersion", Value::Int(version as i64), None);
            if version == 0 {
                if let (Some(created), Some(modified), Some(scale), Some(duration)) =
                    (at32(p, 4), at32(p, 8), at32(p, 12), at32(p, 16))
                {
                    push("MediaCreateDate", Value::Str(qt_date(created)), None);
                    push("MediaModifyDate", Value::Str(qt_date(modified)), None);
                    push("MediaTimeScale", Value::Int(scale as i64), None);
                    push("MediaDuration", Value::Str(seconds(duration, scale)), None);
                }
                if let Some(language) = at16(p, 20) {
                    push(
                        "MediaLanguageCode",
                        Value::Str(language_code(language)),
                        None,
                    );
                }
            }
        }
        if let Some(hdlr) = child(mdia.payload, b"hdlr")
            && let Some(kind) = hdlr.payload.get(8..12)
        {
            let handler = match kind {
                b"vide" => "Video Track",
                b"soun" => "Audio Track",
                b"meta" => "NRT Metadata",
                _ => "Unknown",
            };
            push("HandlerType", Value::Str(handler.to_string()), None);
        }
        if let Some(minf) = child(mdia.payload, b"minf")
            && let Some(stbl) = child(minf.payload, b"stbl")
        {
            let mut compressor = None;
            if let Some(stsd) = child(stbl.payload, b"stsd")
                && let Some(entry) = boxes(stsd.payload.get(8..).unwrap_or_default()).first()
            {
                let kind = String::from_utf8_lossy(&entry.kind).to_string();
                compressor = Some(kind.clone());
                if kind == "CTMD" {
                    push("MetaFormat", Value::Str(kind), None);
                } else {
                    push("GraphicsMode", Value::Str("srcCopy".to_string()), None);
                    push("OpColor", Value::Str("0 0 0".to_string()), None);
                    push("CompressorID", Value::Str(kind), None);
                    let e = entry.payload;
                    if let (Some(width), Some(height)) = (at16(e, 24), at16(e, 26)) {
                        push("SourceImageWidth", Value::Int(width as i64), None);
                        push("SourceImageHeight", Value::Int(height as i64), None);
                    }
                    if let Some(xres) = at32(e, 28) {
                        push(
                            "XResolution",
                            Value::Int((xres as f64 / 65536.0).round() as i64),
                            None,
                        );
                    }
                    if let Some(yres) = at32(e, 32) {
                        push(
                            "YResolution",
                            Value::Int((yres as f64 / 65536.0).round() as i64),
                            None,
                        );
                    }
                    if let Some(depth) = at16(e, 74) {
                        push("BitDepth", Value::Int(depth as i64), None);
                    }
                }
            }
            if let Some(stts) = child(stbl.payload, b"stts")
                && let (Some(count), Some(delta)) = (at32(stts.payload, 4), at32(stts.payload, 8))
                && count > 0
                && delta > 0
            {
                let scale = movie_timescale(data);
                if scale > 0 {
                    push(
                        "VideoFrameRate",
                        Value::Float(scale as f64 / delta as f64),
                        None,
                    );
                }
            }
            if group == "Track1"
                && let Some(stts) = child(stbl.payload, b"stts")
                && let Some(delta) = at32(stts.payload, 12)
            {
                let scale = movie_timescale(data);
                if scale > 0 && delta > 0 {
                    push("SampleTime", Value::Str("0 s".to_string()), None);
                    push("SampleDuration", Value::Str(seconds(delta, scale)), None);
                }
            }
            if compressor.as_deref() == Some("CRAW")
                && let Some(sample) = first_sample(stbl.payload, data)
            {
                push("JpgFromRaw", Value::Bytes(sample.to_vec()), None);
            }
        }
    }
}

fn movie_timescale(data: &[u8]) -> u32 {
    child(data, b"moov")
        .and_then(|moov| child(moov.payload, b"mvhd"))
        .and_then(|mvhd| {
            let version = mvhd.payload.first().copied().unwrap_or(0);
            at32(mvhd.payload, if version == 1 { 20 } else { 12 })
        })
        .unwrap_or(0)
}

fn language_code(raw: u16) -> String {
    let mut text = String::new();
    for shift in [10, 5, 0] {
        let value = ((raw >> shift) & 0x1F) as u8;
        text.push((value + 0x60) as char);
    }
    text
}

fn first_sample<'a>(stbl: &[u8], data: &'a [u8]) -> Option<&'a [u8]> {
    let stsz = child(stbl, b"stsz")?;
    let size = at32(stsz.payload, 4);
    let count = at32(stsz.payload, 8).unwrap_or(0);
    if count == 0 {
        return None;
    }
    let size = match size {
        Some(0) => at32(stsz.payload, 12)?,
        Some(size) => size,
        None => return None,
    } as usize;
    let co64 = child(stbl, b"co64")?;
    let offset = co64.payload.get(8..16).map(be64)? as usize;
    if size == 0 || size > 1024 * 1024 {
        return None;
    }
    data.get(offset..offset.checked_add(size)?)
}
