//! Streaming and batch API tests.

use std::io;
use std::path::PathBuf;

use etng_core::json::Emitter;
use etng_core::{FileReport, ReadOptions, SystemFields, Tag, Value, read_paths};

fn report(index: usize) -> FileReport {
    let mut report = FileReport::new(PathBuf::from(format!("/tmp/photo-{index}.jpg")));
    report.system = SystemFields {
        file_name: format!("photo-{index}.jpg"),
        directory: "/tmp".to_string(),
        file_size: 26_831,
        ..Default::default()
    };
    report.tags.push(Tag::new(
        "EXIF",
        "IFD0",
        "Make",
        Value::Str("Canon".to_string()),
    ));
    report
}

fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = status
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    pages * 4
}

#[test]
fn streaming_writer_keeps_memory_bounded() {
    let mut emitter = Emitter::new(io::sink(), ReadOptions::default());
    emitter.begin().unwrap();
    for index in 0..100_000 {
        emitter.record(&report(index)).unwrap();
    }
    emitter.finish().unwrap();
    let rss = rss_kb();
    assert!(rss < 100 * 1024, "peak RSS {rss} kB is too high");
}

#[test]
fn read_paths_emits_in_order() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/synthetic");
    let paths = vec![base.join("base.jpg"), base.join("base.tif")];
    let mut seen = Vec::new();
    read_paths(&paths, &ReadOptions::default(), |report| {
        seen.push(report.source.clone());
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, paths);
}
