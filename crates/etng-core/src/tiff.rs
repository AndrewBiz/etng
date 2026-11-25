//! Minimal bounds-checked TIFF/IFD walker.
//!
//! Used for format sniffing and for MakerNote blocks that `kamadak-exif` does
//! not descend into. Every offset is checked against the buffer; a broken file
//! yields `None` instead of a panic.

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub tag: u16,
    pub typ: u16,
    pub count: u32,
    pub value_or_offset: u32,
    pub entry_offset: usize,
}

#[derive(Debug)]
pub struct Ifd {
    pub entries: Vec<Entry>,
    pub next: u32,
}

#[derive(Clone, Copy)]
pub struct Tiff<'a> {
    pub data: &'a [u8],
    pub le: bool,
    pub base: usize,
}

impl<'a> Tiff<'a> {
    /// Parse a TIFF header at `base`.
    pub fn at(data: &'a [u8], base: usize) -> Option<Self> {
        let magic = data.get(base..base + 4)?;
        let le = match magic {
            b"II*\0" => true,
            b"MM\0*" => false,
            _ => return None,
        };
        Some(Self { data, le, base })
    }

    /// Parse a TIFF header at the start of the buffer.
    pub fn new(data: &'a [u8]) -> Option<Self> {
        Self::at(data, 0)
    }

    /// Treat `data[note_start..]` as an IFD block with no TIFF header.
    pub fn from_ifd(data: &'a [u8], note_start: usize, le: bool) -> Self {
        Self {
            data,
            le,
            base: note_start,
        }
    }

    pub fn u16_at(&self, offset: usize) -> Option<u16> {
        let bytes = self.data.get(offset..offset + 2)?;
        Some(if self.le {
            u16::from_le_bytes([bytes[0], bytes[1]])
        } else {
            u16::from_be_bytes([bytes[0], bytes[1]])
        })
    }

    pub fn u32_at(&self, offset: usize) -> Option<u32> {
        let bytes = self.data.get(offset..offset + 4)?;
        Some(if self.le {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        })
    }

    /// Offset of the first IFD, relative to the TIFF header.
    pub fn first_ifd_offset(&self) -> Option<u32> {
        self.u32_at(self.base + 4)
    }

    /// Read an IFD located at `offset` relative to `self.base`.
    pub fn read_ifd(&self, offset: u32) -> Option<Ifd> {
        let limit = 1024;
        let start = self.base.checked_add(offset as usize)?;
        let count = self.u16_at(start)? as usize;
        if count > limit {
            return None;
        }
        let mut entries = Vec::with_capacity(count.min(256));
        for i in 0..count {
            let entry_offset = start.checked_add(2 + i * 12)?;
            let tag = self.u16_at(entry_offset)?;
            let typ = self.u16_at(entry_offset + 2)?;
            let count = self.u32_at(entry_offset + 4)?;
            let value_or_offset = self.u32_at(entry_offset + 8)?;
            entries.push(Entry {
                tag,
                typ,
                count,
                value_or_offset,
                entry_offset,
            });
        }
        let next_offset = start.checked_add(2 + count * 12)?;
        let next = self.u32_at(next_offset).unwrap_or(0);
        Some(Ifd { entries, next })
    }

    /// Raw bytes of an entry value, following the out-of-line offset if needed.
    pub fn entry_bytes(&self, entry: &Entry) -> Option<&'a [u8]> {
        let size = type_size(entry.typ)?;
        let len = (size as u64).checked_mul(entry.count as u64)?;
        if len > 64 * 1024 * 1024 {
            return None;
        }
        let len = len as usize;
        if len <= 4 {
            let start = entry.entry_offset + 8;
            return self.data.get(start..start + len);
        }
        let start = self.base.checked_add(entry.value_or_offset as usize)?;
        self.data.get(start..start.checked_add(len)?)
    }

    /// First ASCII value of a tag in the first IFD.
    pub fn first_ascii(&self, tag: u16) -> Option<String> {
        let ifd = self.read_ifd(self.first_ifd_offset()?)?;
        let entry = ifd.entries.iter().find(|e| e.tag == tag)?;
        let bytes = self.entry_bytes(entry)?;
        let text = String::from_utf8_lossy(bytes);
        Some(text.trim_end_matches('\0').trim().to_string())
    }

    pub fn first_u32(&self, tag: u16) -> Option<u32> {
        let ifd = self.read_ifd(self.first_ifd_offset()?)?;
        let entry = ifd.entries.iter().find(|e| e.tag == tag)?;
        let bytes = self.entry_bytes(entry)?;
        if bytes.len() < 4 {
            return None;
        }
        Some(if self.le {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        })
    }
}

fn type_size(typ: u16) -> Option<usize> {
    Some(match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Little-endian TIFF with one IFD holding Make = "ETNG".
    fn tiny_tiff() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"II*\0");
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // one entry
        data.extend_from_slice(&271u16.to_le_bytes()); // Make
        data.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        data.extend_from_slice(&5u32.to_le_bytes()); // count
        data.extend_from_slice(&30u32.to_le_bytes()); // value offset
        data.extend_from_slice(&0u32.to_le_bytes()); // next IFD
        data.extend_from_slice(&0u32.to_le_bytes()); // padding
        data.extend_from_slice(b"ETNG\0");
        data
    }

    #[test]
    fn reads_ascii_tag() {
        let data = tiny_tiff();
        let tiff = Tiff::new(&data).expect("header");
        assert_eq!(tiff.first_ascii(271).as_deref(), Some("ETNG"));
    }

    #[test]
    fn rejects_cyclic_offsets() {
        let mut data = tiny_tiff();
        data[4..8].copy_from_slice(&9_000_000u32.to_le_bytes());
        let tiff = Tiff::new(&data).expect("header");
        assert!(tiff.read_ifd(tiff.first_ifd_offset().unwrap()).is_none());
    }
}
