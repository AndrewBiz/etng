//! XMP and IPTC date updates for JPEG sidecar metadata.
//!
//! The EXIF writer handles the TIFF payload. This module updates matching
//! XMP properties and IPTC IIM datasets in the surrounding JPEG segments.

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";
const IPTC_RESOURCE: u16 = 0x0404;

pub fn update(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let mut out = data.to_vec();
    let mut changed = false;
    if let Some(next) = update_xmp(&out, dates)? {
        out = next;
        changed = true;
    }
    if let Some(next) = update_iptc(&out, dates)? {
        out = next;
        changed = true;
    }
    Ok(changed.then_some(out))
}

fn find_segment(data: &[u8], marker: u8, signature: &[u8]) -> Option<(usize, usize)> {
    let mut pos = 2usize;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let mut marker_pos = pos + 1;
        while marker_pos < data.len() && data[marker_pos] == 0xFF {
            marker_pos += 1;
        }
        let code = *data.get(marker_pos)?;
        pos = marker_pos + 1;
        if code == 0xD9 || code == 0xDA {
            break;
        }
        if code == 0x01 || (0xD0..=0xD7).contains(&code) {
            continue;
        }
        let length = u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]) as usize;
        if length < 2 || pos + length > data.len() {
            break;
        }
        if code == marker && data[pos + 2..pos + length].starts_with(signature) {
            return Some((pos, length));
        }
        pos += length;
    }
    None
}

fn replace_segment(
    data: &[u8],
    length_pos: usize,
    length: usize,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let new_length = payload.len() + 2;
    if new_length > u16::MAX as usize {
        return Err("metadata segment too large after date update".to_string());
    }
    let mut out = Vec::with_capacity(data.len() + payload.len());
    out.extend_from_slice(&data[..length_pos]);
    out.extend_from_slice(&(new_length as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&data[length_pos + length..]);
    Ok(out)
}

fn update_xmp(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let Some((marker, length)) = find_segment(data, 0xE1, XMP_SIG) else {
        return Ok(None);
    };
    let payload = &data[marker + 2..marker + length];
    let xml_bytes = payload
        .strip_prefix(XMP_SIG)
        .ok_or_else(|| "invalid XMP segment".to_string())?;
    let mut xml = String::from_utf8(xml_bytes.to_vec())
        .map_err(|_| "XMP packet is not valid UTF-8".to_string())?;
    let mut changed = false;
    for (name, value) in dates {
        let property = match name.as_str() {
            "DateTimeOriginal" => "photoshop:DateCreated",
            "CreateDate" => "xmp:CreateDate",
            "ModifyDate" => "xmp:ModifyDate",
            _ => continue,
        };
        changed |= set_xmp_value(&mut xml, property, &iso_date(value));
    }
    if !changed {
        return Ok(None);
    }
    let mut new_payload = XMP_SIG.to_vec();
    new_payload.extend_from_slice(xml.as_bytes());
    replace_segment(data, marker, length, &new_payload).map(Some)
}

fn iso_date(value: &str) -> String {
    match value.split_once(' ') {
        Some((date, time)) => format!("{}T{time}", date.replace(':', "-")),
        None => value.to_string(),
    }
}

fn set_xmp_value(xml: &mut String, property: &str, value: &str) -> bool {
    let open = format!("<{property}>");
    if let Some(start) = xml.find(&open) {
        let content = start + open.len();
        if let Some(close) = xml[content..].find(&format!("</{property}>")) {
            xml.replace_range(content..content + close, value);
            return true;
        }
    }
    let attribute = format!("{property}=\"");
    if let Some(start) = xml.find(&attribute) {
        let content = start + attribute.len();
        if let Some(close) = xml[content..].find('"') {
            xml.replace_range(content..content + close, value);
            return true;
        }
    }
    // Add the property to an existing rdf:Description that declares its namespace.
    let prefix = property.split(':').next().unwrap_or(property);
    let namespace = format!("xmlns:{prefix}=");
    let Some(ns_pos) = xml.find(&namespace) else {
        return false;
    };
    let Some(close) = xml[ns_pos..].find("</rdf:Description>") else {
        return false;
    };
    let at = ns_pos + close;
    let insert = format!("\n  <{property}>{value}</{property}>\n ");
    xml.insert_str(at, &insert);
    true
}

fn update_iptc(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let Some((marker, length)) = find_segment(data, 0xED, PHOTOSHOP_SIG) else {
        return Ok(None);
    };
    let payload = &data[marker + 2..marker + length];
    let block = payload
        .strip_prefix(PHOTOSHOP_SIG)
        .ok_or_else(|| "invalid Photoshop segment".to_string())?;
    let Some((iim_start, iim_len)) = find_iptc_resource(block) else {
        return Ok(None);
    };
    let mut updated = block.to_vec();
    let iim = &block[iim_start..iim_start + iim_len];
    let mut changed = false;
    for (name, value) in dates {
        let (date_dataset, time_dataset) = match name.as_str() {
            "DateTimeOriginal" => (Some(55), Some(60)),
            "CreateDate" => (Some(62), Some(63)),
            _ => (None, None),
        };
        if let Some(dataset) = date_dataset {
            changed |= set_iim_value(
                &mut updated,
                iim_start,
                iim_len,
                iim,
                dataset,
                &iptc_date(value),
            );
        }
        if let Some(dataset) = time_dataset {
            let time = iptc_time(value, iim_time_suffix(iim, dataset));
            changed |= set_iim_value(&mut updated, iim_start, iim_len, iim, dataset, &time);
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut new_payload = PHOTOSHOP_SIG.to_vec();
    new_payload.extend_from_slice(&updated);
    replace_segment(data, marker, length, &new_payload).map(Some)
}

fn find_iptc_resource(block: &[u8]) -> Option<(usize, usize)> {
    let mut pos = 0usize;
    while pos + 12 <= block.len() {
        if &block[pos..pos + 4] != b"8BIM" {
            pos += 1;
            continue;
        }
        let resource = u16::from_be_bytes([block[pos + 4], block[pos + 5]]);
        let name_len = block[pos + 6] as usize;
        let name_total = if name_len.is_multiple_of(2) {
            name_len + 2
        } else {
            name_len + 1
        };
        let size_pos = pos + 6 + name_total;
        if size_pos + 4 > block.len() {
            return None;
        }
        let size = u32::from_be_bytes([
            block[size_pos],
            block[size_pos + 1],
            block[size_pos + 2],
            block[size_pos + 3],
        ]) as usize;
        let start = size_pos + 4;
        let end = start.saturating_add(size);
        if end > block.len() {
            return None;
        }
        if resource == IPTC_RESOURCE {
            return Some((start, size));
        }
        pos = end + (size & 1);
    }
    None
}

fn set_iim_value(
    block: &mut [u8],
    iim_start: usize,
    iim_len: usize,
    iim: &[u8],
    dataset: u8,
    value: &str,
) -> bool {
    let mut pos = 0usize;
    while pos + 5 <= iim_len {
        if iim[pos] != 0x1C {
            pos += 1;
            continue;
        }
        let record = iim[pos + 1];
        let current = iim[pos + 2];
        let size = u16::from_be_bytes([iim[pos + 3], iim[pos + 4]]) as usize;
        let start = pos + 5;
        let end = start.saturating_add(size);
        if end > iim_len {
            return false;
        }
        if record == 2 && current == dataset && value.len() == size {
            block[iim_start + start..iim_start + end].copy_from_slice(value.as_bytes());
            return true;
        }
        pos = end;
    }
    false
}

fn iim_time_suffix(iim: &[u8], dataset: u8) -> String {
    let mut pos = 0usize;
    while pos + 5 <= iim.len() {
        if iim[pos] != 0x1C {
            pos += 1;
            continue;
        }
        let size = u16::from_be_bytes([iim[pos + 3], iim[pos + 4]]) as usize;
        let start = pos + 5;
        let end = start.saturating_add(size);
        if end > iim.len() {
            break;
        }
        if iim[pos + 1] == 2 && iim[pos + 2] == dataset && size >= 11 {
            let suffix = &iim[start + 6..end];
            if suffix.first() == Some(&b'+') || suffix.first() == Some(&b'-') {
                return String::from_utf8_lossy(suffix).to_string();
            }
            return "+00:00".to_string();
        }
        pos = end;
    }
    "+00:00".to_string()
}

fn iptc_date(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .take(8)
        .collect()
}

fn iptc_time(value: &str, suffix: String) -> String {
    let digits: String = value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .skip(8)
        .take(6)
        .collect();
    if suffix.starts_with('+') || suffix.starts_with('-') {
        if suffix.len() == 6 {
            let mut parts = suffix.split(':');
            let head = parts.next().unwrap_or("+00");
            let sign = &head[..1.min(head.len())];
            let hours = &head[1..];
            let minutes = parts.next().unwrap_or("00");
            return format!("{digits}{sign}{hours}{minutes}");
        }
        return format!("{digits}{suffix}");
    }
    format!("{digits}+0000")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_formats() {
        assert_eq!(iso_date("2024:05:06 07:08:09"), "2024-05-06T07:08:09");
        assert_eq!(iptc_date("2024:05:06 07:08:09"), "20240506");
        assert_eq!(
            iptc_time("2024:05:06 07:08:09", "+02:00".to_string()),
            "070809+0200"
        );
    }

    #[test]
    fn replaces_element_value() {
        let mut xml = "<rdf:Description><photoshop:DateCreated>2003-03-03T03:03:03</photoshop:DateCreated></rdf:Description>".to_string();
        assert!(set_xmp_value(
            &mut xml,
            "photoshop:DateCreated",
            "2024-05-06T07:08:09"
        ));
        assert!(xml.contains("2024-05-06T07:08:09"));
    }
}
