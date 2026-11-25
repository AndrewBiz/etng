//! Reading a single file into a `FileReport`.

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::formatting::{format_datetime, format_permissions, format_unix};
use crate::jpeg;
use crate::model::{FileReport, SystemFields, Tag, Value};
use crate::options::ReadOptions;
use crate::sniff::{self, FileType};
use crate::{exif, heic, iptc, xmp};

/// Read metadata for one file. Never panics: parser panics are converted into
/// a per-file `error`.
pub fn read_file(path: &Path, opts: &ReadOptions) -> FileReport {
    let mut report = FileReport::new(path.to_path_buf());
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    report.system = system_fields(path, &metadata);

    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    let mapped = unsafe { Mmap::map(&file) };
    let owned = if mapped.is_err() {
        fs::read(path).ok()
    } else {
        None
    };
    let data: &[u8] = match &mapped {
        Ok(map) => map,
        Err(_) => owned.as_deref().unwrap_or(&[]),
    };

    let file_type = sniff::detect_with_hint(data, path.extension().and_then(|ext| ext.to_str()));
    report.system.file_type = file_type.name().to_string();
    report.system.file_type_extension = file_type.extension().to_string();
    report.system.mime_type = file_type.mime().to_string();

    if !file_type.supported() {
        report.error = Some("Unknown file type".to_string());
        return report;
    }

    match catch_unwind(AssertUnwindSafe(|| parse(data, file_type, opts))) {
        Ok((tags, warnings, error)) => {
            report.tags = tags;
            report.warnings = warnings;
            report.error = error;
        }
        Err(_) => {
            report.error = Some("Internal parser error".to_string());
        }
    }
    report
}

/// Read many files in order, invoking `emit` for each report.
pub fn read_paths<F>(paths: &[PathBuf], opts: &ReadOptions, mut emit: F) -> std::io::Result<()>
where
    F: FnMut(&FileReport) -> std::io::Result<()>,
{
    for path in paths {
        let report = read_file(path, opts);
        emit(&report)?;
    }
    Ok(())
}

type ParseResult = (Vec<Tag>, Vec<String>, Option<String>);

fn parse(data: &[u8], file_type: FileType, opts: &ReadOptions) -> ParseResult {
    match file_type {
        FileType::Jpeg => parse_jpeg(data, opts),
        FileType::Cr3 => crate::cr3::parse(data, opts),
        FileType::Tiff | FileType::Dng | FileType::Cr2 | FileType::Nef | FileType::Arw => {
            parse_tiff(data, file_type, opts)
        }
        FileType::Heic => parse_heic(data, opts),
        FileType::Unknown => (
            Vec::new(),
            Vec::new(),
            Some("Unknown file type".to_string()),
        ),
    }
}

fn parse_jpeg(data: &[u8], opts: &ReadOptions) -> ParseResult {
    let info = jpeg::scan(data);
    let mut tags = Vec::new();
    let mut warnings = Vec::new();
    if !info.complete {
        warnings.push("JPEG format error".to_string());
    }

    if let Some(width) = info.width {
        tags.push(Tag::new(
            "File",
            "File",
            "ImageWidth",
            Value::Int(width as i64),
        ));
    }
    if let Some(height) = info.height {
        tags.push(Tag::new(
            "File",
            "File",
            "ImageHeight",
            Value::Int(height as i64),
        ));
    }
    if let Some(encoding) = info.encoding {
        let code = match encoding {
            "Baseline DCT, Huffman coding" => 0,
            "Extended sequential DCT, Huffman coding" => 1,
            "Progressive DCT, Huffman coding" => 2,
            _ => 0,
        };
        tags.push(
            Tag::new("File", "File", "EncodingProcess", Value::Int(code)).with_print(encoding),
        );
    }
    if let Some(precision) = info.precision {
        tags.push(Tag::new(
            "File",
            "File",
            "BitsPerSample",
            Value::Int(precision as i64),
        ));
    }
    if let Some(components) = info.components {
        tags.push(Tag::new(
            "File",
            "File",
            "ColorComponents",
            Value::Int(components as i64),
        ));
    }
    if let Some(sampling) = info.sampling {
        let horizontal = sampling >> 4;
        let vertical = sampling & 0x0F;
        let label = match (horizontal, vertical) {
            (2, 2) => "YCbCr4:2:0 (2 2)",
            (2, 1) => "YCbCr4:2:2 (2 1)",
            (1, 1) => "YCbCr4:4:4 (1 1)",
            _ => "Unknown",
        };
        tags.push(
            Tag::new(
                "File",
                "File",
                "YCbCrSubSampling",
                Value::Str(format!("{horizontal} {vertical}")),
            )
            .with_print(label),
        );
    }
    if let Some(jfif) = &info.jfif {
        tags.push(Tag::new(
            "JFIF",
            "JFIF",
            "JFIFVersion",
            Value::Float(jfif.version.parse().unwrap_or(1.0)),
        ));
        let unit_label = match jfif.units {
            0 => Some("None"),
            1 => Some("inches"),
            2 => Some("cm"),
            3 => Some("none"),
            _ => None,
        };
        let mut unit = Tag::new(
            "JFIF",
            "JFIF",
            "ResolutionUnit",
            Value::Int(jfif.units as i64),
        );
        if let Some(label) = unit_label {
            unit = unit.with_print(label);
        }
        tags.push(unit);
        tags.push(Tag::new(
            "JFIF",
            "JFIF",
            "XResolution",
            Value::Int(jfif.xres as i64),
        ));
        tags.push(Tag::new(
            "JFIF",
            "JFIF",
            "YResolution",
            Value::Int(jfif.yres as i64),
        ));
    }

    if let Some(exif_payload) = info.exif {
        let little_endian = exif_payload.starts_with(b"II");
        push_byte_order(&mut tags, little_endian);
        exif::parse(
            exif_payload,
            little_endian,
            &mut tags,
            &mut warnings,
            opts.numeric,
        );
        push_embedded_image(
            &mut tags,
            exif_payload,
            "IFD1",
            "ThumbnailOffset",
            "ThumbnailLength",
            "ThumbnailImage",
        );
    }
    if let Some(photoshop) = info.photoshop {
        if let Some(iim) = iptc::extract_iim(photoshop) {
            use md5::{Digest, Md5};
            let digest = Md5::digest(iim);
            let hex = digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            tags.push(Tag::new(
                "File",
                "File",
                "CurrentIPTCDigest",
                Value::Str(hex),
            ));
        }
        iptc::parse_photoshop(photoshop, &mut tags, &mut warnings);
    }
    if let Some(xmp_payload) = info.xmp {
        xmp::parse(xmp_payload, &mut tags, &mut warnings);
    }

    push_aperture(&mut tags);
    push_composite(&mut tags, info.width, info.height);
    push_post_composites(&mut tags);
    (tags, warnings, None)
}

fn parse_tiff(data: &[u8], file_type: FileType, opts: &ReadOptions) -> ParseResult {
    let mut tags = Vec::new();
    let mut warnings = Vec::new();
    let little_endian = data.starts_with(b"II");
    push_byte_order(&mut tags, little_endian);
    exif::parse(data, little_endian, &mut tags, &mut warnings, opts.numeric);
    push_embedded_image(
        &mut tags,
        data,
        "IFD1",
        "ThumbnailOffset",
        "ThumbnailLength",
        "ThumbnailImage",
    );
    if file_type == FileType::Cr2 {
        adjust_cr2(&mut tags, data);
    }
    let width = find_int(&tags, "ImageWidth");
    let height = find_int(&tags, "ImageHeight");
    push_aperture(&mut tags);
    push_composite(&mut tags, width, height);
    push_post_composites(&mut tags);
    (tags, warnings, None)
}

fn parse_heic(data: &[u8], opts: &ReadOptions) -> ParseResult {
    let mut tags = Vec::new();
    let mut warnings = Vec::new();
    let dimensions = heic::dimensions(data);
    match heic::extract_exif(data) {
        Some(payload) => {
            let little_endian = payload.starts_with(b"II");
            push_byte_order(&mut tags, little_endian);
            exif::parse(
                payload,
                little_endian,
                &mut tags,
                &mut warnings,
                opts.numeric,
            );
            push_embedded_image(
                &mut tags,
                payload,
                "IFD1",
                "ThumbnailOffset",
                "ThumbnailLength",
                "ThumbnailImage",
            );
        }
        None => warnings.push("HEIC: EXIF item not found".to_string()),
    }
    for packet in heic::extract_xmps(data) {
        xmp::parse(packet, &mut tags, &mut warnings);
    }
    let width = find_int(&tags, "PixelXDimension")
        .or_else(|| dimensions.map(|(w, _)| w))
        .or_else(|| find_int(&tags, "ImageWidth"));
    let height = find_int(&tags, "PixelYDimension")
        .or_else(|| dimensions.map(|(_, h)| h))
        .or_else(|| find_int(&tags, "ImageHeight"));
    if let Some(width) = width {
        tags.push(Tag::new(
            "File",
            "File",
            "ImageWidth",
            Value::Int(width as i64),
        ));
    }
    if let Some(height) = height {
        tags.push(Tag::new(
            "File",
            "File",
            "ImageHeight",
            Value::Int(height as i64),
        ));
    }
    push_aperture(&mut tags);
    push_composite(&mut tags, width, height);
    push_post_composites(&mut tags);
    (tags, warnings, None)
}

fn find_int(tags: &[Tag], name: &str) -> Option<u32> {
    tags.iter()
        .find(|tag| tag.name == name)
        .and_then(|tag| tag.value.as_i64())
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn push_composite(tags: &mut Vec<Tag>, width: Option<u32>, height: Option<u32>) {
    let (Some(width), Some(height)) = (width, height) else {
        return;
    };
    if width == 0 || height == 0 {
        return;
    }
    tags.push(Tag::new(
        "Composite",
        "Composite",
        "ImageSize",
        Value::Str(format!("{width}x{height}")),
    ));
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    let rounded = if megapixels >= 10.0 {
        (megapixels * 10.0).round() / 10.0
    } else {
        (megapixels * 1000.0).round() / 1000.0
    };
    tags.push(Tag::new(
        "Composite",
        "Composite",
        "Megapixels",
        Value::Float(rounded),
    ));
}

/// CR2 reuses TIFF tags 273/279 for the preview image and hides the TIFF
/// bits-per-sample tag, matching ExifTool's CR2 interpretation.
fn adjust_cr2(tags: &mut Vec<Tag>, data: &[u8]) {
    tags.retain(|tag| !(tag.group1 == "IFD0" && tag.name == "BitsPerSample"));
    for tag in tags.iter_mut() {
        if tag.group1 != "IFD0" {
            continue;
        }
        match tag.name {
            "StripOffsets" => tag.name = "PreviewImageStart",
            "StripByteCounts" => tag.name = "PreviewImageLength",
            _ => {}
        }
    }
    push_embedded_image(
        tags,
        data,
        "IFD0",
        "PreviewImageStart",
        "PreviewImageLength",
        "PreviewImage",
    );
}

/// Insert an embedded image payload after its length tag. Offsets are relative
/// to `data` (the TIFF payload for JPEG/HEIC, the file for TIFF/CR2).
fn push_embedded_image(
    tags: &mut Vec<Tag>,
    data: &[u8],
    group1: &'static str,
    offset_name: &str,
    length_name: &str,
    output_name: &'static str,
) {
    let (Some(offset), Some(length)) = (
        find_u32_in(tags, group1, offset_name),
        find_u32_in(tags, group1, length_name),
    ) else {
        return;
    };
    let start = offset as usize;
    let Some(end) = start.checked_add(length as usize) else {
        return;
    };
    let Some(bytes) = data.get(start..end) else {
        return;
    };
    if bytes.is_empty() {
        return;
    }
    let position = tags
        .iter()
        .rposition(|tag| tag.group1 == group1 && tag.name == length_name)
        .map(|index| index + 1)
        .unwrap_or(tags.len());
    tags.insert(
        position,
        Tag::new("EXIF", group1, output_name, Value::Bytes(bytes.to_vec())),
    );
}

fn find_u32_in(tags: &[Tag], group1: &str, name: &str) -> Option<u32> {
    tags.iter()
        .find(|tag| tag.group1 == group1 && tag.name == name)
        .and_then(|tag| tag.value.as_i64())
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn push_byte_order(tags: &mut Vec<Tag>, little_endian: bool) {
    tags.push(
        Tag::new(
            "File",
            "File",
            "ExifByteOrder",
            Value::Str(if little_endian { "II" } else { "MM" }.to_string()),
        )
        .with_print(if little_endian {
            "Little-endian (Intel, II)"
        } else {
            "Big-endian (Motorola, MM)"
        }),
    );
}

pub(crate) fn push_aperture(tags: &mut Vec<Tag>) {
    if let Some(fnumber) = find_f64(tags, "FNumber") {
        let rounded = (fnumber * 10.0).round() / 10.0;
        tags.push(Tag::new(
            "Composite",
            "Composite",
            "Aperture",
            Value::Float(rounded),
        ));
    }
}

pub(crate) fn push_post_composites(tags: &mut Vec<Tag>) {
    if let Some(exposure) = tags
        .iter()
        .find(|tag| tag.name == "ExposureTime")
        .map(|tag| {
            (
                tag.value.as_f64(),
                tag.print.clone().unwrap_or_else(|| tag.value.display()),
            )
        })
        && let Some(value) = exposure.0
    {
        tags.push(
            Tag::new(
                "Composite",
                "Composite",
                "ShutterSpeed",
                Value::Float(value),
            )
            .with_print(exposure.1),
        );
    }

    let latitude = find_f64(tags, "GPSLatitude");
    let longitude = find_f64(tags, "GPSLongitude");
    let latitude_ref = find_str(tags, "GPSLatitudeRef")
        .and_then(|text| text.chars().next())
        .unwrap_or('N');
    let longitude_ref = find_str(tags, "GPSLongitudeRef")
        .and_then(|text| text.chars().next())
        .unwrap_or('E');

    let mut latitude_print = None;
    let mut longitude_print = None;
    if let Some(value) = latitude {
        let print = exif::coordinate_with_ref(value, latitude_ref);
        latitude_print = Some(print.clone());
        tags.push(
            Tag::new("Composite", "Composite", "GPSLatitude", Value::Float(value))
                .with_print(print),
        );
    }
    if let Some(value) = longitude {
        let print = exif::coordinate_with_ref(value, longitude_ref);
        longitude_print = Some(print.clone());
        tags.push(
            Tag::new(
                "Composite",
                "Composite",
                "GPSLongitude",
                Value::Float(value),
            )
            .with_print(print),
        );
    }
    if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
        let machine = format!(
            "{} {}",
            crate::formatting::format_float(latitude),
            crate::formatting::format_float(longitude)
        );
        let mut tag = Tag::new("Composite", "Composite", "GPSPosition", Value::Str(machine));
        if let (Some(lat), Some(lon)) = (latitude_print, longitude_print) {
            tag = tag.with_print(format!("{lat}, {lon}"));
        }
        tags.push(tag);
    }

    if let Some(focal) = find_f64(tags, "FocalLength") {
        let text = match find_f64(tags, "FocalLengthIn35mmFormat") {
            Some(equivalent) => format!("{:.1} mm (35 mm equivalent: {:.1} mm)", focal, equivalent),
            None => format!("{focal:.1} mm"),
        };
        tags.push(Tag::new(
            "Composite",
            "Composite",
            "FocalLength35efl",
            Value::Str(text),
        ));
    }
    if let Some(lens) = find_str(tags, "LensModel") {
        let lens = if lens.starts_with("Canon ") {
            lens
        } else {
            format!("Canon {lens}")
        };
        tags.push(Tag::new(
            "Composite",
            "Composite",
            "LensID",
            Value::Str(lens),
        ));
    }

    if let (Some(date), Some(time)) = (
        find_str(tags, "GPSDateStamp"),
        tags.iter()
            .find(|tag| tag.name == "GPSTimeStamp")
            .map(|tag| tag.print.clone().unwrap_or_else(|| tag.value.display())),
    ) {
        tags.push(Tag::new(
            "Composite",
            "Composite",
            "GPSDateTime",
            Value::Str(format!("{date} {time}Z")),
        ));
    }

    let aperture = find_f64(tags, "FNumber");
    let exposure = find_f64(tags, "ExposureTime");
    let iso = find_f64(tags, "ISO").or_else(|| find_f64(tags, "ISOSpeedRatings"));
    if let (Some(aperture), Some(exposure), Some(iso)) = (aperture, exposure, iso)
        && aperture > 0.0
        && exposure > 0.0
        && iso > 0.0
    {
        let light_value = (aperture * aperture / exposure).log2() - (iso / 100.0).log2();
        let rounded = (light_value * 10.0).round() / 10.0;
        tags.push(
            Tag::new(
                "Composite",
                "Composite",
                "LightValue",
                Value::Float(rounded),
            )
            .with_print(format!("{rounded:.1}")),
        );
    }
}

fn find_f64(tags: &[Tag], name: &str) -> Option<f64> {
    tags.iter()
        .find(|tag| tag.group1 == "ExifIFD" && tag.name == name)
        .or_else(|| tags.iter().find(|tag| tag.name == name))
        .and_then(|tag| tag.value.as_f64())
}

fn find_str(tags: &[Tag], name: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.group1 == "ExifIFD" && tag.name == name)
        .or_else(|| tags.iter().find(|tag| tag.name == name))
        .and_then(|tag| match &tag.value {
            Value::Str(text) => Some(text.clone()),
            _ => None,
        })
}

fn system_fields(path: &Path, metadata: &fs::Metadata) -> SystemFields {
    use std::os::unix::fs::MetadataExt;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let directory = path
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default();
    SystemFields {
        file_name,
        directory,
        file_size: metadata.len(),
        file_modify_date: format_datetime(metadata.modified().ok()),
        file_access_date: format_datetime(metadata.accessed().ok()),
        file_inode_change_date: format_unix(metadata.ctime(), metadata.ctime_nsec() as u32),
        file_permissions: format_permissions(metadata.mode() & 0o7777, metadata.is_dir()),
        file_mode: metadata.mode(),
        file_type: String::new(),
        file_type_extension: String::new(),
        mime_type: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("etng-{name}-{}", std::process::id()));
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn unknown_file_reports_error() {
        let path = write_temp("unknown", b"not an image");
        let report = read_file(&path, &ReadOptions::default());
        assert!(report.error.is_some());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn empty_file_is_safe() {
        let path = write_temp("empty", b"");
        let report = read_file(&path, &ReadOptions::default());
        assert!(report.error.is_some());
        let _ = fs::remove_file(path);
    }
}
