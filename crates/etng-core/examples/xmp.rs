//! Debug helper: list XMP packets in a HEIC file and the tags parsed from them.
//!
//! Usage: cargo run -p etng-core --example xmp -- path/to/file.heic

fn main() {
    let path = match std::env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: xmp <file.heic>");
            std::process::exit(2);
        }
    };
    let data = std::fs::read(&path).expect("read file");
    let packets = etng_core::heic::extract_xmps(&data);
    println!("{} XMP packet(s)", packets.len());
    for (index, packet) in packets.iter().enumerate() {
        let mut tags = Vec::new();
        let mut warnings = Vec::new();
        etng_core::xmp::parse(packet, &mut tags, &mut warnings);
        println!(
            "packet {index}: {} bytes, {} tags, warnings: {warnings:?}",
            packet.len(),
            tags.len()
        );
        for tag in &tags {
            println!("  {}:{} = {:?}", tag.group1, tag.name, tag.value);
        }
        if index == 0 {
            dump_attributes(packet);
        }
    }
}

fn dump_attributes(packet: &[u8]) {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_reader(packet);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                if name == "rdf:Description" {
                    for attribute in element.attributes().with_checks(false).flatten() {
                        println!(
                            "  attr {:?} = {:?}",
                            String::from_utf8_lossy(attribute.key.as_ref()),
                            attribute.unescape_value().map(|v| v.to_string())
                        );
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                println!("  xml error: {err}");
                break;
            }
            _ => {}
        }
        buffer.clear();
    }
}
