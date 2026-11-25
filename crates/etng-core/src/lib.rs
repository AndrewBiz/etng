//! etng metadata reading engine.
//!
//! Public API:
//! - [`read_file`] — read one file into a [`FileReport`]
//! - [`read_paths`] — sequential batch with a per-file callback
//! - [`json::Emitter`] — streaming writer producing `exiftool -j`-compatible JSON
//!
//! ```
//! use etng_core::{ReadOptions, read_file};
//! use std::path::Path;
//!
//! let report = read_file(Path::new("photo.jpg"), &ReadOptions::default());
//! println!("{} tags", report.tags.len());
//! ```

pub mod base64;
pub mod compare;
pub mod compat;
pub mod csv;
pub mod exif;
pub mod formatting;
pub mod heic;
pub mod iptc;
pub mod jpeg;
pub mod json;
pub mod makernotes;
pub mod model;
pub mod options;
pub mod read;
pub mod sniff;
pub mod system;
pub mod tiff;
pub mod xmp;

pub use model::{FileReport, SystemFields, Tag, Value};
pub use options::{FallbackMode, GroupMode, ReadOptions};
pub use read::{read_file, read_paths};
