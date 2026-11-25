//! Compatibility target constants.

/// The ExifTool version whose output contract etng targets.
pub const EXIFTOOL_COMPAT_VERSION: &str = "13.55";

/// etng's own version.
pub const IMPLEMENTATION_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Value reported in the `ExifToolVersion` JSON field.
pub const REPORTED_EXIFTOOL_VERSION: &str = EXIFTOOL_COMPAT_VERSION;
