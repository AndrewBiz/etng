//! Value formatting helpers that mirror ExifTool's textual conventions.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};

pub fn format_float(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    // ExifTool uses `%.10g`: ten significant digits, trailing zeros trimmed.
    let text = if value.abs() >= 1.0 {
        let digits_before = value.abs().log10().floor() as i32 + 1;
        let decimals = (10 - digits_before).max(0) as usize;
        format!("{value:.decimals$}")
    } else {
        format!("{value:.10}")
    };
    let mut trimmed = text;
    if trimmed.contains('.') {
        while trimmed.ends_with('0') {
            trimmed.pop();
        }
        if trimmed.ends_with('.') {
            trimmed.pop();
        }
    }
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed
    }
}

/// ExifTool renders small files as bytes and larger ones with decimal units.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "kB", "MB", "GB", "TB"];
    if bytes < 2000 {
        return format!("{bytes} {}", UNITS[0]);
    }
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let text = if value < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    format!("{text} {}", UNITS[unit])
}

/// `YYYY:MM:DD HH:MM:SS+HH:MM` in local time, as ExifTool prints it.
pub fn format_datetime(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let dt: DateTime<Local> = DateTime::<Local>::from(time);
    dt.format("%Y:%m:%d %H:%M:%S%:z").to_string()
}

/// Format a 32-bit version word as four dot-separated bytes (`0.1.0.0`).
pub fn format_version(raw: u32) -> String {
    raw.to_be_bytes()
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

pub fn format_unix(seconds: i64, nanos: u32) -> String {
    let base = if seconds >= 0 {
        UNIX_EPOCH + std::time::Duration::new(seconds as u64, nanos)
    } else {
        UNIX_EPOCH - std::time::Duration::new(seconds.unsigned_abs(), 0)
    };
    format_datetime(Some(base))
}

/// `-rw-r--r--` style permissions.
pub fn format_permissions(mode: u32, is_dir: bool) -> String {
    let mut out = String::with_capacity(10);
    out.push(if is_dir { 'd' } else { '-' });
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (mask, ch) in bits {
        out.push(if mode & mask != 0 { ch } else { '-' });
    }
    out
}

/// Turn ISO-8601 XMP dates into ExifTool's EXIF-style date strings.
pub fn normalize_xmp_date(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    let looks_iso = bytes.len() >= 10 && bytes[4] == b'-' && bytes[7] == b'-';
    if !looks_iso {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.to_string();
    out.replace_range(7..8, ":");
    out.replace_range(4..5, ":");
    if out.len() > 10 && out.as_bytes()[10] == b'T' {
        out.replace_range(10..11, " ");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_exiftool() {
        assert_eq!(human_size(26_831), "27 kB");
        assert_eq!(human_size(1_500), "1500 bytes");
        assert_eq!(human_size(2_000), "2.0 kB");
        assert_eq!(human_size(512), "512 bytes");
        assert_eq!(human_size(9_386), "9.4 kB");
    }

    #[test]
    fn permissions_are_posix() {
        assert_eq!(format_permissions(0o644, false), "-rw-r--r--");
        assert_eq!(format_permissions(0o755, true), "drwxr-xr-x");
    }

    #[test]
    fn xmp_dates_convert() {
        assert_eq!(
            normalize_xmp_date("2023-07-09T20:36:33+08:00"),
            "2023:07:09 20:36:33+08:00"
        );
        assert_eq!(
            normalize_xmp_date("2023:07:09 20:36:33"),
            "2023:07:09 20:36:33"
        );
    }

    #[test]
    fn floats_avoid_exponents() {
        assert_eq!(format_float(2.8), "2.8");
        assert_eq!(format_float(24.0), "24");
    }
}
