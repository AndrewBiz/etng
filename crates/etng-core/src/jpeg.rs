//! JPEG marker scanner.

#[derive(Clone, Debug)]
pub struct Jfif {
    pub version: String,
    pub units: u8,
    pub xres: u16,
    pub yres: u16,
}

#[derive(Clone, Debug, Default)]
pub struct JpegInfo<'a> {
    pub exif: Option<&'a [u8]>,
    pub xmp: Option<&'a [u8]>,
    /// Photoshop APP13 payload (starts with `8BIM` resources).
    pub photoshop: Option<&'a [u8]>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub precision: Option<u8>,
    pub components: Option<u8>,
    pub sampling: Option<u8>,
    pub encoding: Option<&'static str>,
    pub jfif: Option<Jfif>,
    /// True when an EOI marker is present after the scan data.
    pub complete: bool,
}

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const EXIF_SIG: &[u8] = b"Exif\0\0";
const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";

pub fn scan(data: &[u8]) -> JpegInfo<'_> {
    let mut info = JpegInfo::default();
    let mut pos = 2usize;
    while pos + 1 < data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let mut marker_pos = pos + 1;
        while marker_pos < data.len() && data[marker_pos] == 0xFF {
            marker_pos += 1;
        }
        let Some(&marker) = data.get(marker_pos) else {
            break;
        };
        pos = marker_pos + 1;
        if marker == 0xD9 {
            info.complete = true;
            break;
        }
        if marker == 0xDA {
            info.complete = data[pos..].windows(2).any(|window| window == [0xFF, 0xD9]);
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let Some(len_bytes) = data.get(pos..pos + 2) else {
            break;
        };
        let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
        if len < 2 {
            break;
        }
        let Some(segment) = data.get(pos + 2..pos + len) else {
            break;
        };
        match marker {
            0xE0 => {
                if segment.starts_with(b"JFIF\0") {
                    info.jfif = parse_jfif(segment);
                }
            }
            0xE1 => {
                if let Some(rest) = segment.strip_prefix(EXIF_SIG) {
                    info.exif.get_or_insert(rest);
                } else if let Some(rest) = segment.strip_prefix(XMP_SIG) {
                    info.xmp.get_or_insert(rest);
                }
            }
            0xED => {
                if let Some(rest) = segment.strip_prefix(PHOTOSHOP_SIG) {
                    info.photoshop.get_or_insert(rest);
                }
            }
            m if is_sof(m) => {
                if info.width.is_none() {
                    parse_sof(m, segment, &mut info);
                }
            }
            _ => {}
        }
        pos += len;
    }
    info
}

fn parse_jfif(segment: &[u8]) -> Option<Jfif> {
    if segment.len() < 12 {
        return None;
    }
    let major = segment[5];
    let minor = segment[6];
    Some(Jfif {
        version: format!("{major}.{minor:02}"),
        units: segment[7],
        xres: u16::from_be_bytes([segment[8], segment[9]]),
        yres: u16::from_be_bytes([segment[10], segment[11]]),
    })
}

fn parse_sof(marker: u8, segment: &[u8], info: &mut JpegInfo<'_>) {
    if segment.len() < 6 {
        return;
    }
    info.precision = Some(segment[0]);
    info.height = Some(u16::from_be_bytes([segment[1], segment[2]]) as u32);
    info.width = Some(u16::from_be_bytes([segment[3], segment[4]]) as u32);
    info.components = Some(segment[5]);
    if segment.len() >= 9 {
        info.sampling = Some(segment[7]);
    }
    info.encoding = Some(match marker {
        0xC0 => "Baseline DCT, Huffman coding",
        0xC1 => "Extended sequential DCT, Huffman coding",
        0xC2 => "Progressive DCT, Huffman coding",
        0xC3 => "Lossless, Huffman coding",
        0xC5 => "Differential sequential DCT, Huffman coding",
        0xC6 => "Differential progressive DCT, Huffman coding",
        0xC7 => "Differential lossless, Huffman coding",
        0xC9 => "Extended sequential DCT, arithmetic coding",
        0xCA => "Progressive DCT, arithmetic coding",
        0xCB => "Lossless, arithmetic coding",
        0xCD => "Differential sequential DCT, arithmetic coding",
        0xCE => "Differential progressive DCT, arithmetic coding",
        0xCF => "Differential lossless, arithmetic coding",
        _ => "Unknown",
    });
}

fn is_sof(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_jpeg() -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        // APP1 Exif
        let payload = b"Exif\0\0II*\0\x08\0\0\0\0\0";
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(payload);
        // SOF0 2x3, 3 components
        let sof = [8u8, 0, 3, 0, 2, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1];
        out.extend_from_slice(&[0xFF, 0xC0]);
        out.extend_from_slice(&((sof.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&sof);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    #[test]
    fn reads_segments() {
        let data = minimal_jpeg();
        let info = scan(&data);
        assert_eq!(info.width, Some(2));
        assert_eq!(info.height, Some(3));
        assert_eq!(info.components, Some(3));
        assert_eq!(info.encoding, Some("Baseline DCT, Huffman coding"));
        assert!(info.exif.is_some());
    }

    #[test]
    fn truncated_file_does_not_panic() {
        let data = minimal_jpeg();
        for cut in 0..data.len() {
            let _ = scan(&data[..cut]);
        }
    }
}
