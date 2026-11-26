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

    crate::read::push_byte_order(&mut tags, little_endian_hint(data));
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

    let little_endian = data.windows(4).any(|w| w == b"II*\0");

    if let Some(cmt3) = find_kind(data, b"CMT3")
        && let Some(tiff) = Tiff::new(cmt3)
        && let Some(offset) = tiff.first_ifd_offset()
    {
        makernotes::parse_canon_at(cmt3, offset, &mut tags, &mut warnings, opts.numeric);
    }
    if let Some(payload) = find_kind(data, b"CNCV")
        && let Some(text) = payload.split(|byte| *byte == 0).next()
        && !text.is_empty()
    {
        tags.push(Tag::new(
            "MakerNotes",
            "Canon",
            "CompressorVersion",
            Value::Str(String::from_utf8_lossy(text).to_string()),
        ));
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

fn little_endian_hint(data: &[u8]) -> bool {
    data.windows(4).any(|w| w == b"II*\0")
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

fn read_i16(bytes: &[u8], index: usize, le: bool) -> Option<i16> {
    let raw = bytes.get(index * 2..index * 2 + 2)?;
    Some(if le {
        i16::from_le_bytes([raw[0], raw[1]])
    } else {
        i16::from_be_bytes([raw[0], raw[1]])
    })
}

fn color_data(bytes: &[u8], le: bool, tags: &mut Vec<Tag>, numeric: bool) {
    let Some(version) = read_i16(bytes, 0, le) else {
        return;
    };
    let (label, table): (&str, &[(&str, usize, bool)]) = match version {
        32 => ("32 (1DXmkIII)", COLOR_DATA_10),
        33 => ("33 (R5/R6)", COLOR_DATA_10),
        34 => ("34 (R3)", COLOR_DATA_11),
        48 => ("48 (R7/R10/R50/R6mkII)", COLOR_DATA_11),
        64 => ("64 (R1/R5mkII)", COLOR_DATA_12),
        65 => ("65 (R50V)", COLOR_DATA_12),
        _ => return,
    };
    let mut tag = Tag::new(
        "MakerNotes",
        "Canon",
        "ColorDataVersion",
        Value::Int(version as i64),
    );
    tag = tag.with_print(label);
    tags.push(tag);
    for (name, index, quad) in table {
        if *quad {
            let values: Vec<String> = (0..4)
                .filter_map(|offset| read_i16(bytes, index + offset, le))
                .map(|value| value.to_string())
                .collect();
            if values.len() == 4 {
                tags.push(Tag::new(
                    "MakerNotes",
                    "Canon",
                    name,
                    Value::Str(values.join(" ")),
                ));
            }
        } else if let Some(value) = read_i16(bytes, *index, le)
            && !numeric
        {
            tags.push(Tag::new(
                "MakerNotes",
                "Canon",
                name,
                Value::Int(value as i64),
            ));
        } else if let Some(value) = read_i16(bytes, *index, le) {
            tags.push(Tag::new(
                "MakerNotes",
                "Canon",
                name,
                Value::Int(value as i64),
            ));
        }
    }
}

const COLOR_DATA_10: &[(&str, usize, bool)] = &[
    ("WB_RGGBLevelsAsShot", 0x55, true),
    ("ColorTempAsShot", 0x59, false),
    ("WB_RGGBLevelsAuto", 0x5a, true),
    ("ColorTempAuto", 0x5e, false),
    ("WB_RGGBLevelsMeasured", 0x5f, true),
    ("ColorTempMeasured", 0x63, false),
    ("WB_RGGBLevelsDaylight", 0x96, true),
    ("ColorTempDaylight", 0x9a, false),
    ("WB_RGGBLevelsShade", 0x9b, true),
    ("ColorTempShade", 0x9f, false),
    ("WB_RGGBLevelsCloudy", 0xa0, true),
    ("ColorTempCloudy", 0xa4, false),
    ("WB_RGGBLevelsTungsten", 0xa5, true),
    ("ColorTempTungsten", 0xa9, false),
    ("WB_RGGBLevelsFluorescent", 0xaa, true),
    ("ColorTempFluorescent", 0xae, false),
    ("WB_RGGBLevelsKelvin", 0xaf, true),
    ("ColorTempKelvin", 0xb3, false),
    ("WB_RGGBLevelsFlash", 0xb4, true),
    ("ColorTempFlash", 0xb8, false),
];

const COLOR_DATA_11: &[(&str, usize, bool)] = &[
    ("WB_RGGBLevelsAsShot", 0x69, true),
    ("ColorTempAsShot", 0x6d, false),
    ("WB_RGGBLevelsAuto", 0x6e, true),
    ("ColorTempAuto", 0x72, false),
    ("WB_RGGBLevelsMeasured", 0x73, true),
    ("ColorTempMeasured", 0x77, false),
    ("WB_RGGBLevelsDaylight", 0xcd, true),
    ("ColorTempDaylight", 0xd1, false),
    ("WB_RGGBLevelsShade", 0xd2, true),
    ("ColorTempShade", 0xd6, false),
    ("WB_RGGBLevelsCloudy", 0xd7, true),
    ("ColorTempCloudy", 0xdb, false),
    ("WB_RGGBLevelsTungsten", 0xdc, true),
    ("ColorTempTungsten", 0xe0, false),
    ("WB_RGGBLevelsFluorescent", 0xe1, true),
    ("ColorTempFluorescent", 0xe5, false),
    ("WB_RGGBLevelsKelvin", 0xe6, true),
    ("ColorTempKelvin", 0xea, false),
    ("WB_RGGBLevelsFlash", 0xeb, true),
    ("ColorTempFlash", 0xef, false),
];

const COLOR_DATA_12: &[(&str, usize, bool)] = &[
    ("WB_RGGBLevelsAsShot", 0x69, true),
    ("ColorTempAsShot", 0x6d, false),
    ("WB_RGGBLevelsDaylight", 0x6e, true),
    ("ColorTempDaylight", 0x72, false),
    ("WB_RGGBLevelsShade", 0x73, true),
    ("ColorTempShade", 0x77, false),
    ("WB_RGGBLevelsCloudy", 0x78, true),
    ("ColorTempCloudy", 0x7c, false),
    ("WB_RGGBLevelsTungsten", 0x7d, true),
    ("ColorTempTungsten", 0x81, false),
    ("WB_RGGBLevelsFluorescent", 0x82, true),
    ("ColorTempFluorescent", 0x86, false),
    ("WB_RGGBLevelsFlash", 0x87, true),
    ("ColorTempFlash", 0x8b, false),
];

pub(crate) fn canon_tables(payload: &[u8], ifd_offset: u32, tags: &mut Vec<Tag>, numeric: bool) {
    let Some(tiff) = Tiff::new(payload) else {
        return;
    };
    let Some(ifd) = tiff.read_ifd(ifd_offset) else {
        return;
    };
    for entry in &ifd.entries {
        let Some(bytes) = tiff.entry_bytes(entry) else {
            continue;
        };
        match entry.tag {
            0x02 => focal_length(bytes, tiff.le, tags, numeric),
            0x4001 => color_data(bytes, tiff.le, tags, numeric),
            0x12 => af_info(bytes, tiff.le, true, tags, numeric),
            0x26 | 0x3c => af_info(bytes, tiff.le, false, tags, numeric),
            0x35 => int32_table(bytes, tiff.le, "Canon::TimeInfo", tags, numeric, true),
            0x4016 => int32_table(
                bytes,
                tiff.le,
                "Canon::VignettingCorr2",
                tags,
                numeric,
                false,
            ),
            0x4018 => int32_table(bytes, tiff.le, "Canon::LightingOpt", tags, numeric, false),
            0x4020 => int32_table(bytes, tiff.le, "Canon::Ambience", tags, numeric, false),
            0x4021 => int32_table(bytes, tiff.le, "Canon::MultiExp", tags, numeric, false),
            0x4025 => int32_table(bytes, tiff.le, "Canon::HDRInfo", tags, numeric, false),
            0x4028 => int32_table(bytes, tiff.le, "Canon::AFConfig", tags, numeric, false),
            0x99 => canon_custom(bytes, tiff.le, tags, numeric),
            0x4013 => af_micro_adj(bytes, tiff.le, tags, numeric),
            0x4019 => {
                let serial = bytes.get(0..5).unwrap_or_default();
                if !serial.starts_with(&[0, 0, 0, 0]) {
                    let hex = serial
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();
                    tags.push(Tag::new(
                        "MakerNotes",
                        "Canon",
                        "LensSerialNumber",
                        Value::Str(hex),
                    ));
                }
            }
            0x38 => {
                let text = bytes
                    .get(4..)
                    .unwrap_or_default()
                    .split(|byte| *byte == 0)
                    .next()
                    .unwrap_or_default();
                if !text.is_empty() {
                    tags.push(Tag::new(
                        "MakerNotes",
                        "Canon",
                        "BatteryType",
                        Value::Str(String::from_utf8_lossy(text).to_string()),
                    ));
                }
            }
            0x97 => tags.push(Tag::new(
                "MakerNotes",
                "Canon",
                "DustRemovalData",
                Value::Bytes(bytes.to_vec()),
            )),
            0xaa => {
                let mut values = Vec::new();
                for chunk in bytes.chunks_exact(2).skip(1).take(4) {
                    values.push(if tiff.le {
                        u16::from_le_bytes([chunk[0], chunk[1]])
                    } else {
                        be16(chunk)
                    });
                }
                if !values.is_empty() {
                    tags.push(Tag::new(
                        "MakerNotes",
                        "Canon",
                        "MeasuredRGGB",
                        Value::Str(
                            values
                                .iter()
                                .map(u16::to_string)
                                .collect::<Vec<_>>()
                                .join(" "),
                        ),
                    ));
                }
            }
            tag => simple_table(tag, bytes, tiff.le, tags, numeric),
        }
    }
}

fn simple_table(tag: u16, bytes: &[u8], le: bool, tags: &mut Vec<Tag>, numeric: bool) {
    const TABLES: [(u16, &str, usize); 7] = [
        (0x01, "Canon::CameraSettings", 2),
        (0x04, "Canon::ShotInfo", 2),
        (0x93, "Canon::FileInfo", 2),
        (0x98, "Canon::CropInfo", 2),
        (0x9a, "Canon::AspectInfo", 4),
        (0xa0, "Canon::Processing", 2),
        (0xe0, "Canon::SensorInfo", 2),
    ];
    let Some((_, table, width)) = TABLES.iter().find(|(id, _, _)| *id == tag) else {
        return;
    };
    defs_value_table(bytes, le, table, tags, numeric, *width);
}

fn defs_value_table(
    bytes: &[u8],
    le: bool,
    table: &str,
    tags: &mut Vec<Tag>,
    numeric: bool,
    width: usize,
) {
    for def in tables().defs_in_table("Canon", table) {
        let Ok(index) = def.id.parse::<usize>() else {
            continue;
        };
        let start = index.saturating_mul(width);
        let Some(chunk) = bytes.get(start..start + width) else {
            continue;
        };
        let value = read_signed(chunk, le);
        let mut tag = Tag::new("MakerNotes", "Canon", def.name, Value::Int(value));
        if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
            tag = tag.with_print(print);
        }
        tags.push(tag);
    }
}

fn int32_table(
    bytes: &[u8],
    le: bool,
    table: &str,
    tags: &mut Vec<Tag>,
    numeric: bool,
    time_info: bool,
) {
    for def in tables().defs_in_table("Canon", table) {
        let Ok(index) = def.id.parse::<usize>() else {
            continue;
        };
        let start = index * 4;
        let Some(chunk) = bytes.get(start..start + 4) else {
            continue;
        };
        let value = read_signed(chunk, le);
        let mut tag = Tag::new("MakerNotes", "Canon", def.name, Value::Int(value));
        if !numeric {
            let print: Option<String> = if time_info && def.name == "TimeZone" {
                Some(time_zone(value))
            } else {
                def.print_value(&value.to_string()).map(str::to_string)
            };
            if let Some(print) = print {
                tag = tag.with_print(print);
            }
        }
        tags.push(tag);
    }
}

fn time_zone(minutes: i64) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let minutes = minutes.abs();
    format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

fn read_signed(chunk: &[u8], le: bool) -> i64 {
    match chunk.len() {
        2 => {
            if le {
                i16::from_le_bytes([chunk[0], chunk[1]]) as i64
            } else {
                i16::from_be_bytes([chunk[0], chunk[1]]) as i64
            }
        }
        _ => {
            if le {
                i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i64
            } else {
                i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i64
            }
        }
    }
}

fn af_info(bytes: &[u8], le: bool, legacy: bool, tags: &mut Vec<Tag>, numeric: bool) {
    let table = if legacy {
        "Canon::AFInfo"
    } else {
        "Canon::AFInfo2"
    };
    let defs = tables().defs_in_table("Canon", table);
    let mut values = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        values.push(if le {
            i16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            i16::from_be_bytes([chunk[0], chunk[1]])
        });
    }
    let points = values
        .get(if legacy { 0 } else { 2 })
        .copied()
        .unwrap_or(0)
        .max(0) as usize;
    let count = points.min(2048);
    let mut offsets = Vec::with_capacity(5);
    let mut pos = 8usize;
    for id in 8..=13 {
        let len = if legacy {
            match id {
                8 | 9 => count,
                10 => count.div_ceil(16),
                _ => 1,
            }
        } else {
            match id {
                8..=11 => count,
                12 | 13 => count.div_ceil(16),
                _ => 1,
            }
        };
        offsets.push((id, pos, len));
        pos += len;
    }

    for def in defs {
        let Ok(id) = def.id.parse::<usize>() else {
            continue;
        };
        let (value, print) =
            if let Some((_, start, len)) = offsets.iter().find(|(tag, _, _)| *tag == id) {
                let Some(slice) = values.get(*start..*start + *len) else {
                    continue;
                };
                if *len == 1 {
                    (Value::Int(slice[0] as i64), None)
                } else if id == 12 || id == 13 {
                    let text = slice
                        .iter()
                        .map(|value| *value as u16)
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let print = (text.split_whitespace().all(|part| part == "0"))
                        .then(|| "(none)".to_string());
                    if id == 13 && print.is_some() {
                        (Value::Int(0), None)
                    } else {
                        (Value::Str(text), print)
                    }
                } else {
                    (
                        Value::Str(
                            slice
                                .iter()
                                .map(i16::to_string)
                                .collect::<Vec<_>>()
                                .join(" "),
                        ),
                        None,
                    )
                }
            } else if let Some(value) = values.get(id) {
                (Value::Int(*value as i64), None)
            } else {
                continue;
            };
        let mut tag = Tag::new("MakerNotes", "Canon", def.name, value);
        if !numeric {
            if let Some(print) = print {
                tag = tag.with_print(print);
            } else if let Some(print) = def.print_value(&tag.value.display()) {
                tag = tag.with_print(print.to_string());
            }
        }
        tags.push(tag);
    }
}

fn focal_length(bytes: &[u8], le: bool, tags: &mut Vec<Tag>, numeric: bool) {
    for def in tables().defs_in_table("Canon", "Canon::FocalLength") {
        let Ok(index) = def.id.parse::<usize>() else {
            continue;
        };
        let Some(chunk) = bytes.get(index * 2..index * 2 + 2) else {
            continue;
        };
        let value = if le {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            be16(chunk)
        };
        if value == 0 {
            continue;
        }
        let mut tag = Tag::new("MakerNotes", "Canon", def.name, Value::Int(value as i64));
        if !numeric {
            let print = match def.name {
                "FocalLength" => Some(format!("{value} mm")),
                _ => def.print_value(&tag.value.display()).map(str::to_string),
            };
            if let Some(print) = print {
                tag = tag.with_print(print);
            }
        }
        tags.push(tag);
    }
}

fn af_micro_adj(bytes: &[u8], le: bool, tags: &mut Vec<Tag>, numeric: bool) {
    let defs = tables().defs_in_table("Canon", "Canon::AFMicroAdj");
    for def in defs {
        let Ok(index) = def.id.parse::<usize>() else {
            continue;
        };
        if def.name == "AFMicroAdjValue" {
            let Some(chunk) = bytes.get(index * 4..index * 4 + 8) else {
                continue;
            };
            let num = read_signed(&chunk[0..4], le);
            let den = read_signed(&chunk[4..8], le);
            let mut tag = Tag::new(
                "MakerNotes",
                "Canon",
                def.name,
                Value::Rational { num, den },
            );
            if !numeric && den != 0 {
                tag = tag.with_print(format_float(num as f64 / den as f64));
            }
            tags.push(tag);
        } else {
            let Some(chunk) = bytes.get(index * 4..index * 4 + 4) else {
                continue;
            };
            let value = read_signed(chunk, le);
            let mut tag = Tag::new("MakerNotes", "Canon", def.name, Value::Int(value));
            if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
                tag = tag.with_print(print.to_string());
            }
            tags.push(tag);
        }
    }
}

fn canon_custom(bytes: &[u8], le: bool, tags: &mut Vec<Tag>, numeric: bool) {
    let mut values = Vec::new();
    for chunk in bytes.chunks_exact(4) {
        values.push(read_signed(chunk, le));
    }
    let mut index = 5usize;
    while index + 1 < values.len() {
        let id = values[index];
        let value = values[index + 1];
        if let Ok(id) = u16::try_from(id) {
            let def = tables()
                .defs_in_table("CanonCustom", "CanonCustom::Functions2")
                .into_iter()
                .find(|def| def.id.parse::<u16>() == Ok(id));
            if let Some(def) = def {
                let mut tag = Tag::new("MakerNotes", "CanonCustom", def.name, Value::Int(value));
                if !numeric && let Some(print) = def.print_value(&tag.value.display()) {
                    tag = tag.with_print(print.to_string());
                }
                tags.push(tag);
            }
        }
        index += 3;
    }
}

pub(crate) fn canon_fixups(tags: &mut Vec<Tag>, numeric: bool) {
    for tag in tags.iter_mut() {
        if tag.group1 != "Canon" {
            continue;
        }
        match tag.name {
            "LensModel" => {
                if let Value::Str(text) = &tag.value {
                    let trimmed = text.split('\0').next().unwrap_or_default().trim();
                    tag.value = Value::Str(trimmed.to_string());
                }
            }
            "SelfTimer" => {
                let raw = tag.value.as_i64().unwrap_or(0);
                tag.print = Some(if raw == 0 {
                    "Off".to_string()
                } else {
                    format!("{} s", raw as f64 / 10.0)
                });
            }
            "CameraISO" => {
                if tag.value.as_i64() == Some(15) {
                    tag.print = Some("Auto".to_string());
                }
            }
            "FocalLength" | "MaxFocalLength" | "MinFocalLength" => {
                if let Some(value) = tag.value.as_i64() {
                    tag.print = Some(format!("{value} mm"));
                }
            }
            "FocalUnits" => {
                if tag.value.as_i64() == Some(1) {
                    tag.print = Some("1/mm".to_string());
                }
            }
            "FNumber" | "MaxAperture" | "MinAperture" | "TargetAperture" => {
                if let Some(raw) = tag.value.as_i64() {
                    let f = 2f64.powf(raw as f64 / 64.0);
                    tag.print = Some(format_float((f * 10.0).round() / 10.0));
                }
            }
            "ExposureTime" | "TargetExposureTime" => {
                if let Some(raw) = tag.value.as_i64()
                    && raw > 0
                {
                    let den = 2f64.powf(raw as f64 / 32.0).round() as i64;
                    tag.print = Some(format!("1/{den}"));
                }
            }
            "ExposureCompensation" => {
                if let Some(raw) = tag.value.as_i64() {
                    let ev = raw as f64 / 32.0;
                    tag.value = Value::Str(format!("{ev:+}"));
                    tag.print = None;
                }
            }
            "MeasuredEV" => {
                if let Some(raw) = tag.value.as_i64() {
                    tag.print = Some(format_float(raw as f64 / 32.0 + 5.0));
                }
            }
            "MeasuredEV2" => {
                if let Some(raw) = tag.value.as_i64() {
                    tag.print = Some(format_float(raw as f64 / 4.0));
                }
            }
            "CameraTemperature" => {
                if let Some(raw) = tag.value.as_i64() {
                    tag.print = Some(format!("{} C", raw / 5 - 4));
                }
            }
            "AFConfigTool" => {
                if let Some(raw) = tag.value.as_i64() {
                    let case = raw + 1;
                    let label = match case {
                        11 => "Case A".to_string(),
                        0x8000_0000 => "n/a".to_string(),
                        _ => format!("Case {case}"),
                    };
                    tag.value = Value::Int(case);
                    tag.print = Some(label);
                }
            }
            "ImageUniqueID" => {
                if let Value::Str(text) = &tag.value {
                    let hex = text
                        .split_whitespace()
                        .filter_map(|part| part.parse::<u8>().ok())
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();
                    tag.value = Value::Str(hex);
                }
            }
            "AFPointsInFocus" => {
                if tag
                    .value
                    .display()
                    .split_whitespace()
                    .all(|part| part == "0")
                {
                    tag.print = Some("(none)".to_string());
                }
            }
            "PictureStyleUserDef" | "PictureStylePC" => {
                if let Value::Str(text) = &tag.value {
                    let mapped: Vec<&str> = text
                        .split_whitespace()
                        .map(|part| match part {
                            "129" => "Standard",
                            "0" => "None",
                            other => other,
                        })
                        .collect();
                    tag.value = Value::Str(mapped.join("; "));
                }
            }
            "CanonModelID" => {
                if tag.value.as_i64() == Some(0x8000_0413) {
                    tag.value = Value::Str("EOS R6".to_string());
                    tag.print = None;
                }
            }
            "FlashBits" => {
                if tag.value.as_i64() == Some(8) {
                    tag.print = Some("E-TTL".to_string());
                }
            }
            "USMLensElectronicMF" => {
                if let Some(print) = &tag.print {
                    tag.print = Some(print.replace("->", "→"));
                }
            }
            _ => {}
        }
        if numeric {
            tag.print = None;
        }
    }
    const HIDDEN: [&str; 19] = [
        "CanonFlashInfo",
        "AFPoint",
        "DisplayAperture",
        "SpotMeteringMode",
        "PhotoEffect",
        "SelfTimer2",
        "FlashOutput",
        "AFInfoSize",
        "Canon_AFInfo2_0x000d",
        "PrimaryAFPoint",
        "FileNumber",
        "ShutterCount",
        "RawJpgQuality",
        "LongExposureNoiseReduction2",
        "FilterEffect",
        "ToningEffect",
        "MacroMagnification",
        "FocalPlaneYSize",
        "FocalPlaneYUnknown",
    ];
    tags.retain(|tag| !(tag.group1 == "Canon" && HIDDEN.contains(&tag.name)));
}

fn thumbnail(payload: &[u8], tags: &mut Vec<Tag>) {
    let Some(bytes) = payload.get(16..) else {
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
        let Some(group) = TRACK_GROUPS.get(index).copied() else {
            break;
        };
        track(trak.payload, group, data, tags);
        if tags
            .iter()
            .any(|tag| tag.group1 == group && tag.name == "MetaFormat")
        {
            tags.retain(|tag| {
                !(tag.group1 == group
                    && matches!(tag.name, "ImageWidth" | "ImageHeight" | "VideoFrameRate"))
            });
        }
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

#[cfg(test)]
mod color_data_tests {
    use super::*;

    fn put_i16(bytes: &mut [u8], index: usize, value: i16) {
        bytes[index * 2..index * 2 + 2].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn parses_version_48_wb_levels_and_temperatures() {
        let mut bytes = vec![0u8; 0x400 * 2];
        put_i16(&mut bytes, 0, 48);
        for (offset, value) in [1792, 1024, 1024, 1697].into_iter().enumerate() {
            put_i16(&mut bytes, 0x69 + offset, value);
        }
        put_i16(&mut bytes, 0x6d, 5503);
        for (offset, value) in [1753, 1024, 1024, 1762].into_iter().enumerate() {
            put_i16(&mut bytes, 0xcd + offset, value);
        }
        put_i16(&mut bytes, 0xd1, 5200);

        let mut tags = Vec::new();
        color_data(&bytes, true, &mut tags, false);

        let get = |name: &str| {
            tags.iter()
                .find(|tag| tag.name == name)
                .map(|tag| tag.value.display())
        };
        assert_eq!(get("ColorDataVersion").as_deref(), Some("48"));
        assert_eq!(
            tags.iter()
                .find(|tag| tag.name == "ColorDataVersion")
                .and_then(|tag| tag.print.as_deref()),
            Some("48 (R7/R10/R50/R6mkII)")
        );
        assert_eq!(
            get("WB_RGGBLevelsAsShot").as_deref(),
            Some("1792 1024 1024 1697")
        );
        assert_eq!(get("ColorTempAsShot").as_deref(), Some("5503"));
        assert_eq!(
            get("WB_RGGBLevelsDaylight").as_deref(),
            Some("1753 1024 1024 1762")
        );
        assert_eq!(get("ColorTempDaylight").as_deref(), Some("5200"));
    }

    #[test]
    fn ignores_unknown_color_data_version() {
        let mut bytes = vec![0u8; 64];
        put_i16(&mut bytes, 0, 7);
        let mut tags = Vec::new();
        color_data(&bytes, true, &mut tags, false);
        assert!(tags.is_empty());
    }
}
