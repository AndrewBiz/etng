//! Typed metadata model shared by all parsers and the JSON writer.

use std::path::PathBuf;

use crate::formatting::format_float;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Rational { num: i64, den: i64 },
    Bytes(Vec<u8>),
}

impl Value {
    /// Default textual rendering (used when no print conversion applies).
    pub fn display(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => format_float(*f),
            Value::Rational { num, den } => {
                if *den == 0 {
                    return format!("{num}/0");
                }
                let (num, den) = reduce(*num, *den);
                if den == 1 {
                    return num.to_string();
                }
                // ExifTool prints most rationals as fractions (`1/250`) but
                // small denominators are shown as decimals (`2.8`, `0.5`).
                if matches!(den, 2 | 4 | 5 | 8 | 10 | 16 | 32) {
                    return format_float(num as f64 / den as f64);
                }
                format!("{num}/{den}")
            }
            Value::Bytes(bytes) => format!(
                "(Binary data {} bytes, use -b option to extract)",
                bytes.len()
            ),
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Rational { num, den } if *den != 0 => Some(*num as f64 / *den as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Rational { num, den } if *den != 0 => Some(*num / *den),
            _ => None,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_) | Value::Float(_) | Value::Rational { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub struct Tag {
    /// Family 0 group (EXIF, MakerNotes, XMP, IPTC, File).
    pub group0: &'static str,
    /// Family 1 group (IFD0, ExifIFD, GPS, XMP-dc, Canon, File, System).
    pub group1: &'static str,
    pub name: &'static str,
    pub value: Value,
    /// Print conversion for non-numeric mode.
    pub print: Option<String>,
}

impl Tag {
    pub fn new(
        group0: &'static str,
        group1: &'static str,
        name: &'static str,
        value: Value,
    ) -> Self {
        Self {
            group0,
            group1,
            name,
            value,
            print: None,
        }
    }

    pub fn with_print(mut self, print: impl Into<String>) -> Self {
        self.print = Some(print.into());
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct SystemFields {
    pub file_name: String,
    pub directory: String,
    pub file_size: u64,
    pub file_modify_date: String,
    pub file_access_date: String,
    pub file_inode_change_date: String,
    pub file_permissions: String,
    pub file_mode: u32,
    pub file_type: String,
    pub file_type_extension: String,
    pub mime_type: String,
}

#[derive(Clone, Debug, Default)]
pub struct FileReport {
    pub source: PathBuf,
    pub system: SystemFields,
    pub tags: Vec<Tag>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl FileReport {
    pub fn new(source: PathBuf) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    pub fn push(&mut self, tag: Tag) {
        self.tags.push(tag);
    }
}

fn reduce(num: i64, den: i64) -> (i64, i64) {
    fn gcd(a: u64, b: u64) -> u64 {
        if b == 0 { a.max(1) } else { gcd(b, a % b) }
    }
    let divisor = gcd(num.unsigned_abs(), den.unsigned_abs()) as i64;
    (num / divisor, den / divisor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rationals_reduce_before_display() {
        let value = Value::Rational {
            num: 100,
            den: 17_100,
        };
        assert_eq!(value.display(), "1/171");
        let value = Value::Rational { num: 28, den: 10 };
        assert_eq!(value.display(), "2.8");
    }
}
