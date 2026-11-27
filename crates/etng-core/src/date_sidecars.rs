//! XMP and IPTC date updates for JPEG sidecar metadata.
//!
//! The EXIF writer handles the TIFF payload. This module updates matching
//! XMP properties and IPTC IIM datasets in the surrounding JPEG segments. If a
//! packet is absent it creates a minimal one containing the requested dates.

const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";
const IPTC_RESOURCE: u16 = 0x0404;
const XMP_NS: &str = "http://ns.adobe.com/xap/1.0/";
const PHOTOSHOP_NS: &str = "http://ns.adobe.com/photoshop/1.0/";

pub fn update(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Ok(None);
    }
    let mut out = data.to_vec();
    let mut changed = false;
    if let Some(next) = update_xmp(&out, dates)? {
        out = next;
        changed = true;
    }
    let mut iptc_changed = false;
    if let Some(next) = update_iptc(&out, dates)? {
        out = next;
        changed = true;
        iptc_changed = true;
    }
    if iptc_changed && let Some(next) = refresh_iptc_digest(&out)? {
        out = next;
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

fn insert_segment(data: &[u8], at: usize, marker: u8, payload: &[u8]) -> Result<Vec<u8>, String> {
    let new_length = payload.len() + 2;
    if new_length > u16::MAX as usize {
        return Err("metadata segment too large".to_string());
    }
    let mut out = Vec::with_capacity(data.len() + payload.len() + 4);
    out.extend_from_slice(&data[..at]);
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&(new_length as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&data[at..]);
    Ok(out)
}

fn xmp_property(name: &str) -> Option<&'static str> {
    match name {
        "DateTimeOriginal" => Some("photoshop:DateCreated"),
        "CreateDate" => Some("xmp:CreateDate"),
        "ModifyDate" => Some("xmp:ModifyDate"),
        _ => None,
    }
}

fn update_xmp(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let Some((marker, length)) = find_segment(data, 0xE1, XMP_SIG) else {
        return create_xmp(data, dates);
    };
    let payload = &data[marker + 2..marker + length];
    let xml_bytes = payload
        .strip_prefix(XMP_SIG)
        .ok_or_else(|| "invalid XMP segment".to_string())?;
    let mut xml = String::from_utf8(xml_bytes.to_vec())
        .map_err(|_| "XMP packet is not valid UTF-8".to_string())?;
    let mut changed = false;
    for (name, value) in dates {
        if let Some(property) = xmp_property(name) {
            changed |= set_xmp_value(&mut xml, property, &iso_date(value));
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut new_payload = XMP_SIG.to_vec();
    new_payload.extend_from_slice(xml.as_bytes());
    replace_segment(data, marker, length, &new_payload).map(Some)
}

fn create_xmp(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let mut properties = String::new();
    for (name, value) in dates {
        let Some(property) = xmp_property(name) else {
            continue;
        };
        properties.push_str(&format!(
            "   <{property}>{}</{property}>\n",
            iso_date(value)
        ));
    }
    if properties.is_empty() {
        return Ok(None);
    }
    let xml = format!(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
         <x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
         <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
         <rdf:Description rdf:about='' xmlns:photoshop='{PHOTOSHOP_NS}' xmlns:xmp='{XMP_NS}'>\n\
         {properties}</rdf:Description>\n\
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end='w'?>"
    );
    let mut payload = XMP_SIG.to_vec();
    payload.extend_from_slice(xml.as_bytes());
    insert_segment(data, 2, 0xE1, &payload).map(Some)
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
    let prefix = property.split(':').next().unwrap_or(property);
    let namespace = format!("xmlns:{prefix}=");
    let existing = xml.find(&namespace).and_then(|ns_pos| {
        xml[ns_pos..]
            .find("</rdf:Description>")
            .map(|close| ns_pos + close)
    });
    if let Some(at) = existing {
        xml.insert_str(at, &format!("\n  <{property}>{value}</{property}>\n "));
        return true;
    }
    let Some(rdf_close) = xml.find("</rdf:RDF>") else {
        return false;
    };
    let uri = match prefix {
        "photoshop" => PHOTOSHOP_NS,
        "xmp" => XMP_NS,
        "dc" => DC_NS,
        "Iptc4xmpCore" => IPTC_CORE_NS,
        _ => return false,
    };
    let description = format!(
        "\n <rdf:Description rdf:about='' xmlns:{prefix}='{uri}'>\n  <{property}>{value}</{property}>\n </rdf:Description>\n"
    );
    xml.insert_str(rdf_close, &description);
    true
}

fn update_iptc(data: &[u8], dates: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    if let Some((marker, length)) = find_segment(data, 0xED, PHOTOSHOP_SIG) {
        let payload = &data[marker + 2..marker + length];
        let block = payload
            .strip_prefix(PHOTOSHOP_SIG)
            .ok_or_else(|| "invalid Photoshop segment".to_string())?;
        let existing =
            find_iptc_resource(block).and_then(|(start, size)| block.get(start..start + size));
        let Some((iim, changed)) = merge_iim(existing.unwrap_or_default(), dates) else {
            return Ok(None);
        };
        if !changed {
            return Ok(None);
        }
        let new_block = rebuild_block(block, &iim)?;
        let mut new_payload = PHOTOSHOP_SIG.to_vec();
        new_payload.extend_from_slice(&new_block);
        return replace_segment(data, marker, length, &new_payload).map(Some);
    }
    let Some((iim, changed)) = merge_iim(&[], dates) else {
        return Ok(None);
    };
    if !changed {
        return Ok(None);
    }
    let mut payload = PHOTOSHOP_SIG.to_vec();
    payload.extend_from_slice(&make_resource(&iim));
    insert_segment(data, 2, 0xED, &payload).map(Some)
}

fn merge_iim(existing: &[u8], dates: &[(String, String)]) -> Option<(Vec<u8>, bool)> {
    let mut records = parse_iim(existing);
    let mut changed = false;
    for (name, value) in dates {
        let (date, time) = match name.as_str() {
            "DateTimeOriginal" => (Some(55u8), Some(60u8)),
            "CreateDate" => (Some(62u8), Some(63u8)),
            _ => (None, None),
        };
        if let Some(dataset) = date {
            changed |= set_record(&mut records, dataset, iptc_date(value).into_bytes());
        }
        if let Some(dataset) = time {
            let suffix = time_suffix(&records, dataset);
            changed |= set_record(&mut records, dataset, iptc_time(value, suffix).into_bytes());
        }
    }
    if !changed {
        return None;
    }
    Some((build_iim(&records), true))
}

fn parse_iim(data: &[u8]) -> Vec<(u8, u8, Vec<u8>)> {
    let mut records = Vec::new();
    let mut pos = 0usize;
    while pos + 5 <= data.len() {
        if data[pos] != 0x1C {
            pos += 1;
            continue;
        }
        let record = data[pos + 1];
        let dataset = data[pos + 2];
        let size = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;
        let start = pos + 5;
        let end = start.saturating_add(size);
        if end > data.len() {
            break;
        }
        records.push((record, dataset, data[start..end].to_vec()));
        pos = end;
    }
    records
}

fn set_record(records: &mut Vec<(u8, u8, Vec<u8>)>, dataset: u8, value: Vec<u8>) -> bool {
    if let Some(record) = records
        .iter_mut()
        .find(|(record, current, _)| *record == 2 && *current == dataset)
    {
        if record.2 == value {
            return false;
        }
        record.2 = value;
        return true;
    }
    records.push((2, dataset, value));
    true
}

fn time_suffix(records: &[(u8, u8, Vec<u8>)], dataset: u8) -> String {
    records
        .iter()
        .find(|(record, current, _)| *record == 2 && *current == dataset)
        .and_then(|(_, _, value)| value.get(6..))
        .filter(|suffix| suffix.first() == Some(&b'+') || suffix.first() == Some(&b'-'))
        .map(|suffix| String::from_utf8_lossy(suffix).to_string())
        .unwrap_or_else(|| "+00:00".to_string())
}

fn build_iim(records: &[(u8, u8, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (record, dataset, value) in records {
        out.extend_from_slice(&[0x1C, *record, *dataset]);
        out.extend_from_slice(&(value.len() as u16).to_be_bytes());
        out.extend_from_slice(value);
    }
    out
}

fn make_resource(iim: &[u8]) -> Vec<u8> {
    make_resource_with_id(IPTC_RESOURCE, iim)
}

fn make_resource_with_id(resource: u16, data: &[u8]) -> Vec<u8> {
    let mut out = b"8BIM".to_vec();
    out.extend_from_slice(&resource.to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
    if data.len() & 1 == 1 {
        out.push(0);
    }
    out
}

#[cfg(test)]
fn find_resource(block: &[u8], wanted: u16) -> Option<(usize, usize)> {
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
        if resource == wanted {
            return Some((start, size));
        }
        pos = end + (size & 1);
    }
    None
}

fn refresh_iptc_digest(data: &[u8]) -> Result<Option<Vec<u8>>, String> {
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
    let iim = &block[iim_start..iim_start + iim_len];
    let digest = {
        use md5::{Digest, Md5};
        Md5::digest(iim)
    };
    let mut new_block = Vec::new();
    let mut pos = 0usize;
    let mut replaced = false;
    while pos + 12 <= block.len() {
        if &block[pos..pos + 4] != b"8BIM" {
            break;
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
            return Err("truncated Photoshop resource".to_string());
        }
        let size = u32::from_be_bytes([
            block[size_pos],
            block[size_pos + 1],
            block[size_pos + 2],
            block[size_pos + 3],
        ]) as usize;
        let end = size_pos + 4 + size;
        if end + (size & 1) > block.len() {
            return Err("truncated Photoshop resource".to_string());
        }
        if resource == 0x0425 {
            new_block.extend_from_slice(&make_resource_with_id(0x0425, &digest));
            replaced = true;
        } else {
            new_block.extend_from_slice(&block[pos..end + (size & 1)]);
        }
        pos = end + (size & 1);
    }
    if !replaced {
        new_block.extend_from_slice(&make_resource_with_id(0x0425, &digest));
    }
    let mut new_payload = PHOTOSHOP_SIG.to_vec();
    new_payload.extend_from_slice(&new_block);
    replace_segment(data, marker, length, &new_payload).map(Some)
}

fn rebuild_block(block: &[u8], iim: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut replaced = false;
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
            return Err("truncated Photoshop resource".to_string());
        }
        let size = u32::from_be_bytes([
            block[size_pos],
            block[size_pos + 1],
            block[size_pos + 2],
            block[size_pos + 3],
        ]) as usize;
        let end = size_pos + 4 + size;
        if end + (size & 1) > block.len() {
            return Err("truncated Photoshop resource".to_string());
        }
        if resource == IPTC_RESOURCE {
            out.extend_from_slice(&make_resource(iim));
            replaced = true;
        } else {
            out.extend_from_slice(&block[pos..end + (size & 1)]);
        }
        pos = end + (size & 1);
    }
    if !replaced {
        out.extend_from_slice(&make_resource(iim));
    }
    Ok(out)
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

const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const IPTC_CORE_NS: &str = "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/";

/// MWG text tags: XMP property, prefix, namespace, optional rdf container.
fn text_xmp(
    name: &str,
) -> Option<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    Some(match name {
        "City" => ("photoshop:City", "photoshop", PHOTOSHOP_NS, None),
        "Country" => ("photoshop:Country", "photoshop", PHOTOSHOP_NS, None),
        "State" => ("photoshop:State", "photoshop", PHOTOSHOP_NS, None),
        "Location" => ("Iptc4xmpCore:Location", "Iptc4xmpCore", IPTC_CORE_NS, None),
        "Keywords" => ("dc:subject", "dc", DC_NS, Some("Bag")),
        "Creator" => ("dc:creator", "dc", DC_NS, Some("Seq")),
        "Copyright" => ("dc:rights", "dc", DC_NS, None),
        _ => return None,
    })
}

fn text_is_array(name: &str) -> bool {
    matches!(name, "Keywords" | "Creator")
}

fn text_dataset(name: &str) -> Option<u8> {
    Some(match name {
        "Keywords" => 25,
        "Creator" => 80,
        "City" => 90,
        "Location" => 92,
        "State" => 95,
        "Country" => 101,
        "Copyright" => 116,
        _ => return None,
    })
}

pub fn update_text(data: &[u8], entries: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Ok(None);
    }
    let mut out = data.to_vec();
    let mut changed = false;
    if let Some(next) = update_xmp_text(&out, entries)? {
        out = next;
        changed = true;
    }
    let mut iptc_changed = false;
    if let Some(next) = update_iptc_text(&out, entries)? {
        out = next;
        changed = true;
        iptc_changed = true;
    }
    if iptc_changed && let Some(next) = refresh_iptc_digest(&out)? {
        out = next;
    }
    Ok(changed.then_some(out))
}

fn update_xmp_text(data: &[u8], entries: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let Some((marker, length)) = find_segment(data, 0xE1, XMP_SIG) else {
        return create_xmp_text(data, entries);
    };
    let payload = &data[marker + 2..marker + length];
    let bytes = payload
        .strip_prefix(XMP_SIG)
        .ok_or_else(|| "invalid XMP segment".to_string())?;
    let mut xml = String::from_utf8(bytes.to_vec())
        .map_err(|_| "XMP packet is not valid UTF-8".to_string())?;
    let mut changed = false;
    for (name, value) in entries {
        if text_is_array(name) {
            continue;
        }
        let Some((property, _, _, _)) = text_xmp(name) else {
            continue;
        };
        changed |= set_xmp_value(&mut xml, property, &escape_xml(value));
    }
    for name in ["Keywords", "Creator"] {
        let values: Vec<String> = entries
            .iter()
            .filter(|(entry_name, _)| entry_name == name)
            .map(|(_, value)| value.clone())
            .collect();
        if values.is_empty() {
            continue;
        }
        let Some((property, prefix, uri, kind)) = text_xmp(name) else {
            continue;
        };
        let kind = kind.unwrap_or("Bag");
        changed |= set_xmp_array(&mut xml, property, prefix, uri, kind, &values);
    }
    if !changed {
        return Ok(None);
    }
    let mut new_payload = XMP_SIG.to_vec();
    new_payload.extend_from_slice(xml.as_bytes());
    replace_segment(data, marker, length, &new_payload).map(Some)
}

fn create_xmp_text(data: &[u8], entries: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    let mut namespaces: Vec<(&str, &str)> = Vec::new();
    let mut body = String::new();
    for (name, value) in entries {
        if text_is_array(name) {
            continue;
        }
        let Some((property, prefix, uri, _)) = text_xmp(name) else {
            continue;
        };
        if !namespaces.iter().any(|(known, _)| *known == prefix) {
            namespaces.push((prefix, uri));
        }
        body.push_str(&format!(
            "   <{property}>{}</{property}>\n",
            escape_xml(value)
        ));
    }
    for name in ["Keywords", "Creator"] {
        let values: Vec<&str> = entries
            .iter()
            .filter(|(entry_name, _)| entry_name == name)
            .map(|(_, value)| value.as_str())
            .collect();
        if values.is_empty() {
            continue;
        }
        let Some((property, prefix, uri, kind)) = text_xmp(name) else {
            continue;
        };
        if !namespaces.iter().any(|(known, _)| *known == prefix) {
            namespaces.push((prefix, uri));
        }
        let kind = kind.unwrap_or("Bag");
        let items: String = values
            .iter()
            .map(|value| format!("<rdf:li>{}</rdf:li>", escape_xml(value)))
            .collect();
        body.push_str(&format!(
            "   <{property}><rdf:{kind}>{items}</rdf:{kind}></{property}>\n"
        ));
    }
    if body.is_empty() {
        return Ok(None);
    }
    let declarations: String = namespaces
        .iter()
        .map(|(prefix, uri)| format!(" xmlns:{prefix}='{uri}'"))
        .collect();
    let xml = format!(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
         <x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
         <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
         <rdf:Description rdf:about=''{declarations}>\n\
         {body}</rdf:Description>\n\
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end='w'?>"
    );
    let mut payload = XMP_SIG.to_vec();
    payload.extend_from_slice(xml.as_bytes());
    insert_segment(data, 2, 0xE1, &payload).map(Some)
}

fn set_xmp_array(
    xml: &mut String,
    property: &str,
    prefix: &str,
    uri: &str,
    kind: &str,
    values: &[String],
) -> bool {
    let items: String = values
        .iter()
        .map(|value| format!("    <rdf:li>{}</rdf:li>\n", escape_xml(value)))
        .collect();
    let content = format!("\n   <rdf:{kind}>\n{items}   </rdf:{kind}>\n  ");
    let open = format!("<{property}");
    if let Some(start) = xml.find(&open) {
        let Some(tag_end) = xml[start..].find('>') else {
            return false;
        };
        let inner = start + tag_end + 1;
        if let Some(close) = xml[inner..].find(&format!("</{property}>")) {
            xml.replace_range(inner..inner + close, &content);
            return true;
        }
    }
    add_xmp_property(xml, property, &content, prefix, uri)
}

fn add_xmp_property(
    xml: &mut String,
    property: &str,
    content: &str,
    prefix: &str,
    uri: &str,
) -> bool {
    let namespace = format!("xmlns:{prefix}=");
    let existing = xml.find(&namespace).and_then(|ns_pos| {
        xml[ns_pos..]
            .find("</rdf:Description>")
            .map(|close| ns_pos + close)
    });
    if let Some(at) = existing {
        xml.insert_str(at, &format!("\n  <{property}>{content}</{property}>\n "));
        return true;
    }
    let Some(rdf_close) = xml.find("</rdf:RDF>") else {
        return false;
    };
    let description = format!(
        "\n <rdf:Description rdf:about='' xmlns:{prefix}='{uri}'>\n  <{property}>{content}</{property}>\n </rdf:Description>\n"
    );
    xml.insert_str(rdf_close, &description);
    true
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn update_iptc_text(data: &[u8], entries: &[(String, String)]) -> Result<Option<Vec<u8>>, String> {
    if let Some((marker, length)) = find_segment(data, 0xED, PHOTOSHOP_SIG) {
        let payload = &data[marker + 2..marker + length];
        let block = payload
            .strip_prefix(PHOTOSHOP_SIG)
            .ok_or_else(|| "invalid Photoshop segment".to_string())?;
        let existing =
            find_iptc_resource(block).and_then(|(start, size)| block.get(start..start + size));
        let Some((iim, changed)) = merge_text_iim(existing.unwrap_or_default(), entries) else {
            return Ok(None);
        };
        if !changed {
            return Ok(None);
        }
        let new_block = rebuild_block(block, &iim)?;
        let mut new_payload = PHOTOSHOP_SIG.to_vec();
        new_payload.extend_from_slice(&new_block);
        return replace_segment(data, marker, length, &new_payload).map(Some);
    }
    let Some((iim, changed)) = merge_text_iim(&[], entries) else {
        return Ok(None);
    };
    if !changed {
        return Ok(None);
    }
    let mut payload = PHOTOSHOP_SIG.to_vec();
    payload.extend_from_slice(&make_resource(&iim));
    insert_segment(data, 2, 0xED, &payload).map(Some)
}

fn merge_text_iim(existing: &[u8], entries: &[(String, String)]) -> Option<(Vec<u8>, bool)> {
    let mut records = parse_iim(existing);
    let mut changed = false;
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for (name, value) in entries {
        if let Some((_, values)) = grouped.iter_mut().find(|(known, _)| known == name) {
            values.push(value.clone());
        } else {
            grouped.push((name.clone(), vec![value.clone()]));
        }
    }
    for (name, values) in grouped {
        let Some(dataset) = text_dataset(&name) else {
            continue;
        };
        if text_is_array(&name) {
            records.retain(|(record, current, _)| !(*record == 2 && *current == dataset));
            for value in values {
                records.push((2, dataset, value.into_bytes()));
            }
            changed = true;
        } else if let Some(value) = values.into_iter().next() {
            changed |= set_record(&mut records, dataset, value.into_bytes());
        }
    }
    if !changed {
        return None;
    }
    Some((build_iim(&records), true))
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

    #[test]
    fn creates_missing_property_description() {
        let mut xml =
            "<rdf:RDF><rdf:Description rdf:about=''></rdf:Description></rdf:RDF>".to_string();
        assert!(set_xmp_value(
            &mut xml,
            "xmp:CreateDate",
            "2024-05-06T07:08:09"
        ));
        assert!(xml.contains("xmlns:xmp="));
        assert!(xml.contains("<xmp:CreateDate>"));
    }

    #[test]
    fn adds_missing_iim_dataset() {
        let mut records = vec![(2, 55, b"20020202".to_vec())];
        assert!(set_record(&mut records, 60, b"070809+0200".to_vec()));
        assert!(build_iim(&records).windows(3).any(|w| w == [0x1C, 2, 60]));
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use md5::{Digest, Md5};

    #[test]
    fn refreshes_iptc_digest_after_date_update() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/phtools/dates/1exif_datetimeoriginal.JPG"
        );
        let data = std::fs::read(path).unwrap();
        let out = update(
            &data,
            &[(
                "DateTimeOriginal".to_string(),
                "2024:05:06 07:08:09".to_string(),
            )],
        )
        .unwrap()
        .unwrap();
        let (marker, length) = find_segment(&out, 0xED, PHOTOSHOP_SIG).unwrap();
        let block = out[marker + 2..marker + length]
            .strip_prefix(PHOTOSHOP_SIG)
            .unwrap();
        let (iim_start, iim_len) = find_iptc_resource(block).unwrap();
        let expected = Md5::digest(&block[iim_start..iim_start + iim_len]);
        let (digest_start, digest_len) = find_resource(block, 0x0425).unwrap();
        assert_eq!(digest_len, 16);
        assert_eq!(&block[digest_start..digest_start + 16], expected.as_slice());
    }
}
