//! End-to-end CLI tests: exit codes, grouping, selection, ordering, fallback.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_etng")
}

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("etng-cli-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn version_reports_compatibility_target() {
    let output = Command::new(bin()).arg("-ver").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "13.55");
}

#[test]
fn help_lists_formats_and_exit_codes() {
    let output = Command::new(bin()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Formats: JPEG"));
    assert!(text.contains("Exit codes"));
}

#[test]
fn unknown_flag_and_zero_threads_exit_2() {
    let output = Command::new(bin())
        .arg("--definitely-unknown")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = Command::new(bin())
        .args(["--threads", "0", "whatever"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn json_selection_limits_fields() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-Make",
            corpus("synthetic/base.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let record = json[0].as_object().unwrap();
    let mut keys: Vec<&str> = record.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["Make", "SourceFile"]);
}

#[test]
fn grouped_output_uses_family_one_prefixes() {
    let output = Command::new(bin())
        .args(["-j", "-G1", corpus("synthetic/base.jpg").to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let record = json[0].as_object().unwrap();
    assert!(record.contains_key("System:FileName"));
    assert!(record.contains_key("IFD0:Make"));
    assert!(record.contains_key("Composite:GPSPosition"));
    assert!(record.get("SourceFile").is_some());
}

#[test]
fn output_order_is_independent_of_thread_count() {
    let first = corpus("synthetic/base.jpg");
    let second = corpus("synthetic/base.tif");
    let run = |threads: &str| {
        let output = Command::new(bin())
            .args([
                "-j",
                "-q",
                "--threads",
                threads,
                first.to_str().unwrap(),
                second.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        json.as_array()
            .unwrap()
            .iter()
            .map(|record| {
                record["SourceFile"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run("1"), run("4"));
}

#[test]
fn warnings_do_not_break_json_stdout() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-q",
            corpus("synthetic/truncated.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn unsupported_file_without_fallback_exits_1() {
    let dir = temp_dir("unsupported");
    let path = dir.join("note.txt");
    fs::write(&path, b"hello").unwrap();
    let output = Command::new(bin())
        .args(["-j", "--fallback=never", path.to_str().unwrap()])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json[0]["Error"]
            .as_str()
            .unwrap()
            .contains("Unknown file type")
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn fallback_always_uses_exiftool_and_maps_fields() {
    let dir = temp_dir("fallback");
    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let script = fake_bin.join("exiftool");
    fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = \"-ver\" ]; then echo 13.55; exit 0; fi\nprintf '[{\"SourceFile\":\"x\",\"IFD0:Make\":\"FakeMake\",\"ExifIFD:Model\":\"FakeModel\"}]\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let input = dir.join("photo.bin");
    fs::write(&input, b"not really an image").unwrap();
    let output = Command::new(bin())
        .args(["-j", "-G1", "--fallback=always", input.to_str().unwrap()])
        .env("PATH", &fake_bin)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["IFD0:Make"], "FakeMake");
    assert_eq!(json[0]["ExifIFD:Model"], "FakeModel");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn missing_exiftool_degrades_with_error() {
    let dir = temp_dir("no-exiftool");
    let path = dir.join("note.txt");
    fs::write(&path, b"hello").unwrap();
    let empty_bin = dir.join("empty");
    fs::create_dir_all(&empty_bin).unwrap();
    let output = Command::new(bin())
        .args(["-j", path.to_str().unwrap()])
        .env("PATH", &empty_bin)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json[0].get("Error").is_some());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exiftool not found"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn env_fallback_mode_is_honoured() {
    let dir = temp_dir("env-fallback");
    let path = dir.join("note.txt");
    fs::write(&path, b"hello").unwrap();
    let empty_bin = dir.join("empty");
    fs::create_dir_all(&empty_bin).unwrap();
    let output = Command::new(bin())
        .args(["-j", path.to_str().unwrap()])
        .env("PATH", &empty_bin)
        .env("ETNG_FALLBACK", "never")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("exiftool not found"),
        "delegation must be disabled by ETNG_FALLBACK=never: {stderr}"
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json[0].get("Error").is_some());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn delegate_receives_literal_path_without_shell() {
    let dir = temp_dir("literal");
    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let log = dir.join("args.log");
    let script = fake_bin.join("exiftool");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"-ver\" ]; then echo 13.55; exit 0; fi\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done >> \"{}\"\nprintf '[{{\"SourceFile\":\"x\",\"Make\":\"M\"}}]\\n'\n",
            log.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let weird = dir.join("a \"b\" c.bin");
    fs::write(&weird, b"data").unwrap();
    let output = Command::new(bin())
        .args(["-j", "--fallback=always", weird.to_str().unwrap()])
        .env("PATH", &fake_bin)
        .output()
        .unwrap();
    assert!(output.status.success());
    let log_text = fs::read_to_string(&log).unwrap();
    assert!(log_text.lines().any(|line| line == weird.to_str().unwrap()));
    let _ = fs::remove_dir_all(dir);
}
