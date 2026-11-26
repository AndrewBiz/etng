//! File type detection by content signature.

use crate::tiff::Tiff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    Jpeg,
    Tiff,
    Dng,
    Cr2,
    Cr3,
    Arw,
    Nef,
    Heic,
    Png,
    Mov,
    Unknown,
}

impl FileType {
    pub fn name(self) -> &'static str {
        match self {
            FileType::Jpeg => "JPEG",
            FileType::Tiff => "TIFF",
            FileType::Dng => "DNG",
            FileType::Cr2 => "CR2",
            FileType::Cr3 => "CR3",
            FileType::Arw => "ARW",
            FileType::Nef => "NEF",
            FileType::Heic => "HEIC",
            FileType::Png => "PNG",
            FileType::Mov => "MOV",
            FileType::Unknown => "Unknown",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            FileType::Jpeg => "jpg",
            FileType::Tiff => "tif",
            FileType::Dng => "dng",
            FileType::Cr2 => "cr2",
            FileType::Cr3 => "cr3",
            FileType::Arw => "arw",
            FileType::Nef => "nef",
            FileType::Heic => "heic",
            FileType::Png => "png",
            FileType::Mov => "mov",
            FileType::Unknown => "",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            FileType::Jpeg => "image/jpeg",
            FileType::Tiff => "image/tiff",
            FileType::Dng => "image/x-adobe-dng",
            FileType::Cr2 => "image/x-canon-cr2",
            FileType::Cr3 => "image/x-canon-cr3",
            FileType::Arw => "image/x-sony-arw",
            FileType::Nef => "image/x-nikon-nef",
            FileType::Heic => "image/heic",
            FileType::Png => "image/png",
            FileType::Mov => "video/quicktime",
            FileType::Unknown => "application/octet-stream",
        }
    }

    pub fn supported(self) -> bool {
        self != FileType::Unknown
    }
}

const HEIC_BRANDS: [&[u8; 4]; 9] = [
    b"heic", b"heix", b"hevc", b"heim", b"heis", b"hevm", b"hevs", b"mif1", b"msf1",
];

pub fn detect(data: &[u8]) -> FileType {
    detect_with_hint(data, None)
}

/// Detect by content, with the file extension used only to disambiguate
/// TIFF-family containers (DNG/NEF/CR2), which share the same signature.
pub fn detect_with_hint(data: &[u8], extension: Option<&str>) -> FileType {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return FileType::Jpeg;
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return FileType::Png;
    }
    if data.len() >= 16 && &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        if HEIC_BRANDS.iter().any(|b| *b == brand) {
            return FileType::Heic;
        }
        if brand == b"crx " {
            return FileType::Cr3;
        }
        if matches!(brand, b"qt  " | b"isom" | b"mp42" | b"3gp4" | b"M4V ") {
            return FileType::Mov;
        }
        return FileType::Unknown;
    }
    if let Some(tiff) = Tiff::new(data) {
        if data.len() >= 12 && &data[8..10] == b"CR" {
            return FileType::Cr2;
        }
        if tiff.first_u32(0xC612).is_some() {
            return FileType::Dng;
        }
        let hint = extension.unwrap_or_default().to_ascii_lowercase();
        if hint == "dng" {
            return FileType::Dng;
        }
        if let Some(make) = tiff.first_ascii(0x010F) {
            let make = make.to_ascii_uppercase();
            if make.contains("NIKON") {
                return FileType::Nef;
            }
            if make.contains("SONY") && hint == "arw" {
                return FileType::Arw;
            }
        }
        if hint == "arw" {
            return FileType::Arw;
        }
        if hint == "nef" {
            return FileType::Nef;
        }
        if hint == "cr2" {
            return FileType::Cr2;
        }
        return FileType::Tiff;
    }
    FileType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jpeg_without_extension() {
        assert_eq!(detect(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), FileType::Jpeg);
    }

    #[test]
    fn rejects_text() {
        assert_eq!(detect(b"just some text"), FileType::Unknown);
    }

    #[test]
    fn detects_tiff() {
        let mut data = Vec::new();
        data.extend_from_slice(b"II*\0");
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(detect(&data), FileType::Tiff);
    }

    #[test]
    fn detects_cr3_brand() {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"ftypcrx \0\0\0\x01crx isom");
        assert_eq!(detect(&data), FileType::Cr3);
    }
}
