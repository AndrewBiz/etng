//! Repository tasks for etng.
//!
//! `cargo xtask generate-tags` — rebuild `crates/etng-tags/data/tags.tsv`
//! from the committed `exiftool -listx` snapshot.
//! `cargo xtask snapshots` — regenerate golden `exiftool -j -G1` snapshots
//! for the test corpus.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

const LISTX: &str = "crates/etng-tags/data/listx.xml";
const TAGS_TSV: &str = "crates/etng-tags/data/tags.tsv";
const CORPUS: &str = "tests/corpus";

const PAIR_SEP: char = '\u{1d}';
const ID_SEP: char = '\u{1e}';

fn main() {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "help".to_string());
    let result = match command.as_str() {
        "generate-tags" => generate_tags(),
        "update-tags" => update_tags(),
        "snapshots" => snapshots(),
        "compare" => compare(),
        _ => {
            eprintln!("usage: cargo xtask <generate-tags|update-tags|snapshots|compare>");
            std::process::exit(2);
        }
    };
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

#[derive(Default)]
struct TableInfo {
    name: String,
    g0: String,
    g1: String,
}

#[derive(Default)]
struct TagInfo {
    id: String,
    name: String,
    typ: String,
    writable: bool,
    g1: String,
    values: Vec<(String, String)>,
}

fn update_tags() -> Result<(), Box<dyn Error>> {
    let version_output = Command::new("exiftool").arg("-ver").output()?;
    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string();
    if version != etng_core::compat::EXIFTOOL_COMPAT_VERSION {
        eprintln!(
            "warning: installed exiftool {version} differs from compat target {}",
            etng_core::compat::EXIFTOOL_COMPAT_VERSION
        );
    }
    let listx = Command::new("exiftool").arg("-listx").arg("-f").output()?;
    if !listx.status.success() {
        return Err("exiftool -listx failed".into());
    }
    fs::write(LISTX, &listx.stdout)?;
    let text = String::from_utf8_lossy(&listx.stdout);
    let tags = text.matches("<tag ").count();
    let tables = text.matches("<table ").count();
    let hash = Command::new("sha256sum").arg(LISTX).output().ok();
    let sha = hash
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default();
    println!("wrote {LISTX}: {tags} tags in {tables} tables (exiftool {version}, sha256 {sha})");
    Ok(())
}

fn generate_tags() -> Result<(), Box<dyn Error>> {
    let listx = Path::new(LISTX);
    if !listx.exists() {
        return Err(format!(
            "{LISTX} is missing; run `exiftool -listx -f > {LISTX}` with the pinned exiftool version"
        )
        .into());
    }

    let mut reader = Reader::from_file(listx)?;
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();

    let mut table: Option<TableInfo> = None;
    let mut tag: Option<TagInfo> = None;
    let mut key_id: Option<String> = None;
    let mut value_lang_ok = false;
    let mut value_text = String::new();

    let mut rows: Vec<String> = Vec::new();
    let mut tables: BTreeSet<String> = BTreeSet::new();

    loop {
        let event = reader.read_event_into(&mut buffer)?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => match e.name().as_ref() {
                b"table" => {
                    let info = TableInfo {
                        name: attr(e, b"name").unwrap_or_default(),
                        g0: attr(e, b"g0").unwrap_or_default(),
                        g1: attr(e, b"g1").unwrap_or_default(),
                    };
                    tables.insert(info.name.clone());
                    if !info.g0.is_empty() && !info.g1.is_empty() {
                        table = Some(info);
                    }
                }
                b"tag" => {
                    if let Some(current_table) = table.as_ref() {
                        let tag_g1 = attr(e, b"g1").filter(|s| !s.is_empty());
                        let info = TagInfo {
                            id: attr(e, b"id").unwrap_or_default(),
                            name: attr(e, b"name").unwrap_or_default(),
                            typ: attr(e, b"type").unwrap_or_default(),
                            writable: attr(e, b"writable").as_deref() == Some("true"),
                            g1: tag_g1.unwrap_or_else(|| current_table.g1.clone()),
                            values: Vec::new(),
                        };
                        if matches!(event, Event::Empty(_)) {
                            push_row(&mut rows, current_table, info);
                        } else {
                            tag = Some(info);
                        }
                    }
                }
                b"key" => key_id = attr(e, b"id"),
                b"val" => {
                    value_lang_ok = attr(e, b"lang").as_deref() == Some("en");
                    value_text.clear();
                }
                _ => {}
            },
            Event::Text(ref e) => {
                if value_lang_ok {
                    value_text.push_str(&e.decode()?);
                }
            }
            Event::End(ref e) => match e.name().as_ref() {
                b"val" => {
                    if value_lang_ok
                        && let (Some(id), Some(current)) = (key_id.clone(), tag.as_mut())
                        && !value_text.trim().is_empty()
                    {
                        current.values.push((id, value_text.trim().to_string()));
                    }
                    value_lang_ok = false;
                }
                b"key" => key_id = None,
                b"tag" => {
                    if let (Some(current_table), Some(info)) = (table.as_ref(), tag.take()) {
                        push_row(&mut rows, current_table, info);
                    }
                }
                b"table" => table = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    let mut out = String::with_capacity(rows.len() * 96);
    for row in &rows {
        out.push_str(row);
        out.push('\n');
    }
    fs::write(TAGS_TSV, out)?;
    println!(
        "generated {TAGS_TSV}: {} tags from {} tables ({} bytes)",
        rows.len(),
        tables.len(),
        fs::metadata(TAGS_TSV)?.len()
    );
    Ok(())
}

fn push_row(rows: &mut Vec<String>, table: &TableInfo, info: TagInfo) {
    if info.id.is_empty() || info.name.is_empty() {
        return;
    }
    let values: Vec<String> = info
        .values
        .iter()
        .map(|(id, label)| format!("{}{ID_SEP}{}", sanitize(id), sanitize(label)))
        .collect();
    rows.push(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        sanitize(&table.g0),
        sanitize(&info.g1),
        sanitize(&table.name),
        sanitize(&info.id),
        sanitize(&info.name),
        sanitize(&info.typ),
        if info.writable { "1" } else { "0" },
        values.join(&PAIR_SEP.to_string())
    ));
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\t' | '\r' | '\n' | '\u{1d}' | '\u{1e}' | '\u{1f}' => ' ',
            other => other,
        })
        .collect()
}

fn attr(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| a.unescape_value().ok())
        .map(|v| v.to_string())
}

fn snapshots() -> Result<(), Box<dyn Error>> {
    let root = Path::new(CORPUS);
    if !root.exists() {
        return Err(format!("{CORPUS} does not exist").into());
    }
    let mut count = 0usize;
    for path in corpus_files(root)? {
        let output = Command::new("exiftool")
            .arg("-j")
            .arg("-G1")
            .arg(&path)
            .output()?;
        if !output.status.success() {
            eprintln!("exiftool failed for {}: {}", path.display(), output.status);
            continue;
        }
        let mut json_path = path.clone().into_os_string();
        json_path.push(".json");
        fs::write(PathBuf::from(json_path), &output.stdout)?;
        count += 1;
    }
    println!("wrote {count} snapshots under {CORPUS}");
    Ok(())
}

fn corpus_files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if name.ends_with(".json")
                || name.starts_with('.')
                || name.ends_with(".md")
                || name.ends_with(".py")
                || name.ends_with(".sh")
            {
                continue;
            }
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Compare `etng -j -G1` against the committed golden snapshots.
fn compare() -> Result<(), Box<dyn Error>> {
    const MASK: [&str; 5] = [
        "FileAccessDate",
        "FileInodeChangeDate",
        "FileModifyDate",
        "FileCreateDate",
        "Warning",
    ];
    const KNOWN_PREFIXES: [&str; 4] = ["Apple:", "QuickTime:", "Meta:", "Vivo:"];
    const SKIP_FILES: [&str; 3] = ["broken.jpg", "endless_loop.jpg", "invalid-gps"];
    const KNOWN_KEYS: [&str; 25] = [
        "Composite:GPSAltitude",
        "Composite:FocalLength35efl",
        "Composite:Flash",
        "Composite:LensID",
        "Composite:RunTimeSincePowerUp",
        "Composite:ScaleFactor35efl",
        "Composite:SubSecCreateDate",
        "Composite:SubSecDateTimeOriginal",
        "Composite:SubSecModifyDate",
        "Composite:CircleOfConfusion",
        "Composite:FOV",
        "Composite:HyperfocalDistance",
        "File:Comment",
        "ExifIFD:MakerNoteUnknownText",
        "IFD1:ThumbnailOffset",
        "XMP-exif:FlashFired",
        "XMP-exif:FlashFunction",
        "XMP-exif:FlashMode",
        "XMP-exif:FlashRedEyeMode",
        "XMP-exif:FlashReturn",
        "XMP-photoshop:History",
        "XMP-xmpMM:DocumentID",
        "XMP-crs:ToneCurve",
        "XMP-tiff:BitsPerSample",
        "XMP-tiff:Orientation",
    ];

    let root = Path::new(CORPUS);
    if !root.exists() {
        return Err(format!("{CORPUS} does not exist").into());
    }
    let mut files = 0usize;
    let mut failures = 0usize;
    for path in corpus_files(root)? {
        if SKIP_FILES
            .iter()
            .any(|name| path.file_name().and_then(|f| f.to_str()) == Some(name))
        {
            println!("{}: skipped (hostile input, documented)", path.display());
            continue;
        }
        let mut snapshot_path = path.clone().into_os_string();
        snapshot_path.push(".json");
        let snapshot_path = PathBuf::from(snapshot_path);
        if !snapshot_path.exists() {
            eprintln!(
                "{}: no snapshot (run `cargo xtask snapshots`)",
                path.display()
            );
            failures += 1;
            continue;
        }
        let expected: serde_json::Value = serde_json::from_slice(&fs::read(&snapshot_path)?)?;
        let expected = expected.get(0).cloned().unwrap_or_default();
        let output = Command::new("target/debug/etng")
            .arg("-j")
            .arg("-G1")
            .arg(&path)
            .output()?;
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let actual = actual.get(0).cloned().unwrap_or_default();
        let report = etng_core::compare::compare_documents(&expected, &actual, &MASK);

        let is_known = |key: &str| {
            KNOWN_PREFIXES.iter().any(|prefix| key.starts_with(prefix)) || KNOWN_KEYS.contains(&key)
        };
        let known = report
            .only_expected
            .iter()
            .chain(report.only_actual.iter())
            .chain(report.changed.iter().map(|diff| &diff.key))
            .filter(|key| is_known(key))
            .count();
        let only_expected: Vec<&String> = report
            .only_expected
            .iter()
            .filter(|key| !is_known(key))
            .collect();
        let only_actual: Vec<&String> = report
            .only_actual
            .iter()
            .filter(|key| !is_known(key))
            .collect();
        let changed: Vec<&etng_core::compare::FieldDiff> = report
            .changed
            .iter()
            .filter(|diff| !is_known(&diff.key))
            .collect();

        println!(
            "{}: +{} etng-only, -{} snapshot-only, ~{} changed, {known} known vendor",
            path.display(),
            only_actual.len(),
            only_expected.len(),
            changed.len()
        );
        for key in &only_actual {
            println!("  etng-only: {key}");
        }
        for key in &only_expected {
            println!("  snapshot-only: {key}");
        }
        for diff in &changed {
            println!(
                "  changed {}: {:?} != {:?}",
                diff.key, diff.actual, diff.expected
            );
        }
        files += 1;
        let mut binary_ok = true;
        if let Some(expected_object) = expected.as_object() {
            for key in expected_object.keys() {
                let tag = key.rsplit(':').next().unwrap_or_default();
                if tag != "ThumbnailImage" && tag != "PreviewImage" {
                    continue;
                }
                // Only EXIF-embedded images are extracted natively; CR3/QuickTime
                // previews come from the fallback path, which `-b` disables.
                if !(key.starts_with("IFD0:") || key.starts_with("IFD1:")) {
                    continue;
                }
                let selection = format!("-{tag}");
                let ours = Command::new("target/debug/etng")
                    .arg("-b")
                    .arg(&selection)
                    .arg(&path)
                    .output()?;
                let theirs = Command::new("exiftool")
                    .arg("-b")
                    .arg(&selection)
                    .arg(&path)
                    .output()?;
                if ours.stdout != theirs.stdout {
                    println!(
                        "  binary {tag}: {} bytes from etng, {} from exiftool",
                        ours.stdout.len(),
                        theirs.stdout.len()
                    );
                    binary_ok = false;
                }
            }
        }
        if !only_expected.is_empty() || !only_actual.is_empty() || !changed.is_empty() || !binary_ok
        {
            failures += 1;
        }
    }
    println!("compared {files} files, {failures} with divergences");
    if failures > 0 {
        return Err(format!("{failures} files diverge from the golden corpus").into());
    }
    Ok(())
}
