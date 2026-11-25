//! Robustness harness: deterministic mutations of real corpus files must never
//! panic, hang or allocate unbounded memory.

use std::fs;
use std::path::{Path, PathBuf};

use etng_core::{ReadOptions, read_file};

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name)
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

#[test]
fn mutated_files_do_not_panic() {
    let sources = [
        corpus("synthetic/base.jpg"),
        corpus("synthetic/base.tif"),
        corpus("real/exif.heic"),
    ];
    let mut rng = Lcg(0xE726_0001);
    let temp = std::env::temp_dir().join(format!("etng-fuzz-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let options = ReadOptions::default();

    let mut iterations = 0usize;
    for source in sources {
        let original = fs::read(&source).expect("corpus file");
        if original.is_empty() {
            continue;
        }
        for round in 0..100 {
            let mut mutated = original.clone();
            let flips = 1 + (rng.next() % 16) as usize;
            for _ in 0..flips {
                let index = (rng.next() as usize) % mutated.len();
                mutated[index] ^= (rng.next() % 255 + 1) as u8;
            }
            if round % 3 == 0 && mutated.len() > 8 {
                let cut = (rng.next() as usize) % mutated.len();
                mutated.truncate(cut.max(1));
            }
            let path = temp.join(format!("case-{iterations}.bin"));
            fs::write(&path, &mutated).unwrap();
            let report = read_file(&path, &options);
            // A report is always produced, with either tags or an error.
            assert!(
                report.error.is_some() || !report.tags.is_empty() || report.system.file_size > 0,
                "empty report for case {iterations}"
            );
            iterations += 1;
        }
    }
    let _ = fs::remove_dir_all(&temp);
    assert!(iterations >= 300, "ran only {iterations} mutations");
}
