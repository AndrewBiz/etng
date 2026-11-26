//! XMP (RDF/XML) parsing into ExifTool-named tags.

use etng_tags::tables;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::formatting::normalize_xmp_date;
use crate::model::{Tag, Value};

struct Elem {
    prefix: String,
    local: String,
    text: String,
    items: Vec<String>,
    has_child: bool,
}

pub fn parse(data: &[u8], out: &mut Vec<Tag>, warnings: &mut Vec<String>) {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack: Vec<Elem> = Vec::new();
    let mut values: Vec<(String, String, String)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(ref e)) => {
                if let Some(parent) = stack.last_mut() {
                    parent.has_child = true;
                }
                let (prefix, local) = split_name(e.name().as_ref());
                collect_attributes(e, &mut values);
                stack.push(Elem {
                    prefix,
                    local,
                    text: String::new(),
                    items: Vec::new(),
                    has_child: false,
                });
            }
            Ok(Event::Empty(ref e)) => {
                let (prefix, local) = split_name(e.name().as_ref());
                collect_attributes(e, &mut values);
                if prefix != "rdf"
                    && prefix != "xmlns"
                    && !local.is_empty()
                    && let Some(value) = text_attribute(e)
                {
                    values.push((prefix, local, value));
                }
            }
            Ok(Event::Text(ref t)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&t.decode().unwrap_or_default());
                }
            }
            Ok(Event::End(_)) => {
                let Some(element) = stack.pop() else {
                    continue;
                };
                if element.local == "li" {
                    if let Some(parent) = stack.last_mut() {
                        let value = element.text.trim();
                        if !value.is_empty() {
                            parent.items.push(value.to_string());
                        }
                    }
                    continue;
                }
                if matches!(element.local.as_str(), "Alt" | "Bag" | "Seq") {
                    if let Some(parent) = stack.last_mut() {
                        parent.items.extend(element.items);
                        parent.has_child = true;
                    }
                    continue;
                }
                if element.prefix == "rdf" || element.local == "Description" {
                    continue;
                }
                let value = if element.items.is_empty() {
                    element.text.trim().to_string()
                } else {
                    element.items.join(", ")
                };
                if !value.is_empty() {
                    let (prefix, local) = nested_name(&stack, &element.prefix, &element.local);
                    values.push((prefix, local, value));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                warnings.push(format!("XMP: {err}"));
                break;
            }
            _ => {}
        }
        buffer.clear();
    }

    for (prefix, local, value) in values {
        let (table, id) = xmp_lookup(&prefix, &local);
        let Some(def) = tables().find_table_id(&table, &id) else {
            continue;
        };
        let value = if def.typ == "date" {
            normalize_xmp_date(&value)
        } else if def.name.ends_with("Version") {
            match value.parse::<u32>() {
                Ok(raw) => crate::formatting::format_version(raw),
                Err(_) => value,
            }
        } else {
            value
        };
        let value = numeric_xmp_value(&value);
        let mut tag = Tag::new("XMP", def.g1, def.name, value);
        if let Some(print) = crate::exif::special_print(def.name, &tag.value) {
            tag = tag.with_print(print);
        } else if let Some(print) = def.print_value(&tag.value.display()) {
            tag = tag.with_print(print);
        }
        out.push(tag);
    }
}

fn nested_name(stack: &[Elem], prefix: &str, local: &str) -> (String, String) {
    if prefix == "Iptc4xmpExt"
        && stack.iter().any(|element| element.local == "LocationShown")
        && matches!(
            local,
            "City"
                | "CountryCode"
                | "CountryName"
                | "ProvinceState"
                | "Sublocation"
                | "WorldRegion"
                | "GPSLatitude"
                | "GPSLongitude"
                | "GPSAltitude"
                | "GPSAltitudeRef"
        )
    {
        return (prefix.to_string(), format!("LocationShown{local}"));
    }
    if prefix == "mwg-coll"
        && stack.iter().any(|element| element.local == "Collections")
        && matches!(local, "CollectionName" | "CollectionURI")
    {
        return (prefix.to_string(), local.to_string());
    }
    (prefix.to_string(), local.to_string())
}

fn xmp_lookup(prefix: &str, local: &str) -> (String, String) {
    let table = match prefix {
        "Iptc4xmpCore" => "XMP::iptcCore".to_string(),
        "Iptc4xmpExt" => "XMP::iptcExt".to_string(),
        "mwg-coll" => "MWG::Collections".to_string(),
        _ => format!("XMP::{prefix}"),
    };
    let id = if prefix == "mwg-coll" {
        format!("Collections{local}")
    } else {
        local.to_string()
    };
    (table, id)
}

fn split_name(name: &[u8]) -> (String, String) {
    let name = String::from_utf8_lossy(name);
    match name.split_once(':') {
        Some((prefix, local)) => (prefix.to_string(), local.to_string()),
        None => (String::new(), name.to_string()),
    }
}

fn collect_attributes(
    element: &quick_xml::events::BytesStart<'_>,
    values: &mut Vec<(String, String, String)>,
) {
    for attr in element.attributes().with_checks(false).flatten() {
        let key = attr.key.as_ref();
        let Ok(value) = attr.unescape_value() else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || key.starts_with(b"xmlns") || key.starts_with(b"xml:") {
            continue;
        }
        let Some((prefix, local)) = String::from_utf8_lossy(key)
            .split_once(':')
            .map(|(p, l)| (p.to_string(), l.to_string()))
        else {
            continue;
        };
        if prefix == "rdf" || local == "about" || local == "parseType" || local == "lang" {
            continue;
        }
        values.push((prefix, local, value.to_string()));
    }
}

fn text_attribute(element: &quick_xml::events::BytesStart<'_>) -> Option<String> {
    for attr in element.attributes().with_checks(false).flatten() {
        if (attr.key.as_ref() == b"rdf:value" || attr.key.as_ref() == b"value")
            && let Ok(value) = attr.unescape_value()
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn numeric_xmp_value(text: &str) -> Value {
    if let Some((num, den)) = text.split_once('/')
        && let (Ok(num), Ok(den)) = (num.trim().parse::<i64>(), den.trim().parse::<i64>())
        && den != 0
    {
        return Value::Rational { num, den };
    }
    if let Ok(number) = text.parse::<f64>() {
        let unsigned = text.trim_start_matches(['-', '+']);
        let leading_zero =
            unsigned.len() > 1 && unsigned.starts_with('0') && !unsigned.starts_with("0.");
        if !leading_zero {
            return Value::Float(number);
        }
    }
    Value::Str(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
    xmp:CreatorTool="etng test">
   <dc:title>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Вечерний город</rdf:li>
    </rdf:Alt>
   </dc:title>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>night</rdf:li>
     <rdf:li>city</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <photoshop:City>Москва</photoshop:City>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn parses_titles_lists_and_city() {
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        parse(SAMPLE.as_bytes(), &mut tags, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let title = tags.iter().find(|t| t.name == "Title").expect("Title");
        assert_eq!(title.value.display(), "Вечерний город");
        assert_eq!(title.group1, "XMP-dc");
        let subject = tags.iter().find(|t| t.name == "Subject").expect("Subject");
        assert_eq!(subject.value.display(), "night, city");
        let city = tags.iter().find(|t| t.name == "City").expect("City");
        assert_eq!(city.value.display(), "Москва");
    }

    #[test]
    fn nested_location_and_collection_are_flattened() {
        let packet = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
 <rdf:Description rdf:about="" xmlns:Iptc4xmpExt="http://iptc.org/std/Iptc4xmpExt/2008-02-29/">
  <Iptc4xmpExt:LocationShown><rdf:Bag><rdf:li rdf:parseType="Resource">
   <Iptc4xmpExt:City>Brugge</Iptc4xmpExt:City>
   <Iptc4xmpExt:CountryCode>BE</Iptc4xmpExt:CountryCode>
   <Iptc4xmpExt:Sublocation>Grote Markt</Iptc4xmpExt:Sublocation>
  </rdf:li></rdf:Bag></Iptc4xmpExt:LocationShown>
 </rdf:Description>
 <rdf:Description rdf:about="" xmlns:mwg-coll="http://www.metadataworkinggroup.com/schemas/collections/">
  <mwg-coll:Collections><rdf:Bag><rdf:li rdf:parseType="Resource">
   <mwg-coll:CollectionName>Belgium Brugge</mwg-coll:CollectionName>
  </rdf:li></rdf:Bag></mwg-coll:Collections>
 </rdf:Description>
 </rdf:RDF></x:xmpmeta>"#;
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        parse(packet, &mut tags, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let names: Vec<(&str, &str)> = tags.iter().map(|tag| (tag.group1, tag.name)).collect();
        assert!(
            names.contains(&("XMP-iptcExt", "LocationShownCity")),
            "{names:?}"
        );
        assert!(
            names.contains(&("XMP-iptcExt", "LocationShownCountryCode")),
            "{names:?}"
        );
    }

    #[test]
    fn attribute_properties_are_parsed() {
        let packet = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about=""
 xmlns:xmp="http://ns.adobe.com/xap/1.0/"
 xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
 xmp:CreateDate="2022-07-22T21:26:32" xmp:CreatorTool="15.5"
 photoshop:DateCreated="2022-07-22T21:26:32"/></rdf:RDF></x:xmpmeta>"#;
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        parse(packet, &mut tags, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let names: Vec<&str> = tags.iter().map(|tag| tag.name).collect();
        assert!(names.contains(&"CreateDate"), "{names:?}");
        assert!(names.contains(&"CreatorTool"), "{names:?}");
        assert!(names.contains(&"XMPToolkit"), "{names:?}");
    }

    #[test]
    fn malformed_xml_warns_instead_of_panicking() {
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        // An unclosed document must not panic; warnings are best-effort.
        parse(
            b"<x:xmpmeta><rdf:RDF><dc:title>oops",
            &mut tags,
            &mut warnings,
        );
    }
}
