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

#[test]
fn binary_scalars_are_raw_and_unseparated() {
    let output = Command::new(bin())
        .args([
            "-b",
            "-Make",
            "-Orientation",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Canon1");
}

#[test]
fn binary_selection_order_follows_arguments() {
    let output = Command::new(bin())
        .args([
            "-b",
            "-Model",
            "-Make",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Canon EOS 40DCanon");
}

#[test]
fn binary_extracts_jpeg_thumbnail() {
    let output = Command::new(bin())
        .args([
            "-b",
            "-ThumbnailImage",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 1378);
    assert_eq!(&output.stdout[..2], &[0xFF, 0xD8]);
    assert_eq!(&output.stdout[output.stdout.len() - 2..], &[0xFF, 0xD9]);
}

#[test]
fn binary_extracts_cr2_preview_bytes() {
    let output = Command::new(bin())
        .args([
            "-b",
            "-PreviewImage",
            corpus("synthetic/cr2_sample.cr2").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.stdout.is_empty());
    let jpeg = fs::read(corpus("synthetic/base.jpg")).unwrap();
    assert_eq!(output.stdout.len(), 9216);
    assert!(output.stdout.starts_with(&jpeg));
}

#[test]
fn binary_json_uses_base64_prefix() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-b",
            "-ThumbnailImage",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let value = json[0]["ThumbnailImage"].as_str().unwrap();
    assert!(value.starts_with("base64:/9j/"), "{value}");
    assert!(value.len() > 1000);
}

#[test]
fn default_json_keeps_binary_placeholder() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-ThumbnailImage",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json[0]["ThumbnailImage"],
        "(Binary data 1378 bytes, use -b option to extract)"
    );
}

#[test]
fn binary_system_values_are_raw() {
    let output = Command::new(bin())
        .args([
            "-b",
            "-FileSize",
            "-FilePermissions",
            "-FileTypeExtension",
            corpus("synthetic/base.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let expected = format!(
        "{}100644JPG",
        fs::metadata(corpus("synthetic/base.jpg")).unwrap().len()
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn binary_errors_stay_off_stdout() {
    let dir = temp_dir("binary-error");
    let path = dir.join("note.txt");
    fs::write(&path, b"hello").unwrap();
    let output = Command::new(bin())
        .args(["-b", "--fallback=never", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown file type"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn csv_selected_columns_follow_arguments_and_case() {
    let output = Command::new(bin())
        .args([
            "-csv",
            "-model",
            "-make",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("SourceFile,Model,Make"));
    assert!(lines.next().unwrap().ends_with(",Canon EOS 40D,Canon"));
}

#[test]
fn csv_group_prefix_and_values() {
    let output = Command::new(bin())
        .args([
            "-csv",
            "-G1",
            "-Make",
            "-Orientation",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("SourceFile,IFD0:Make,IFD0:Orientation"));
    assert!(
        lines
            .next()
            .unwrap()
            .ends_with(",Canon,Horizontal (normal)")
    );
}

#[test]
fn csv_union_header_is_alphabetical_and_missing_cells_are_empty() {
    let output = Command::new(bin())
        .args([
            "-csv",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
            corpus("synthetic/base.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    assert_eq!(header[0], "SourceFile");
    let columns = &header[1..];
    let mut sorted = columns.to_vec();
    sorted.sort_unstable();
    assert_eq!(columns, sorted.as_slice());
    assert!(columns.contains(&"City"));
    assert!(columns.contains(&"Make"));

    let base_row = lines
        .find(|line| line.starts_with(corpus("synthetic/base.jpg").to_str().unwrap()))
        .unwrap();
    let fields: Vec<&str> = base_row.split(',').collect();
    let city = header.iter().position(|column| *column == "City").unwrap();
    assert!(fields[city].contains("Москва"));
}

#[test]
fn csv_quotes_values_with_commas_and_quotes() {
    let output = Command::new(bin())
        .args([
            "-csv",
            "-ComponentsConfiguration",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"Y, Cb, Cr, -\""), "{text}");
}

#[test]
fn csv_binary_values_use_base64() {
    let output = Command::new(bin())
        .args([
            "-csv",
            "-b",
            "-ThumbnailImage",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(",base64:/9j/"), "{text}");
}

#[test]
fn csv_numeric_mode_and_precedence_over_json() {
    let output = Command::new(bin())
        .args([
            "-csv",
            "-j",
            "-n",
            "-Orientation",
            "-FileSize",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("SourceFile,Orientation,FileSize"));
    let row = lines.next().unwrap();
    assert!(row.ends_with(",1,7958"), "{row}");
}

#[test]
fn exclude_removes_fields_and_columns() {
    let file = corpus("real/Canon_40D.jpg");
    let json = Command::new(bin())
        .args([
            "-j",
            "-Make",
            "-Model",
            "-x",
            "Model",
            file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert!(parsed[0].get("Make").is_some());
    assert!(parsed[0].get("Model").is_none());

    let csv = Command::new(bin())
        .args([
            "-csv",
            "-Make",
            "-Model",
            "-x",
            "Model",
            file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text = String::from_utf8(csv.stdout).unwrap();
    assert_eq!(text.lines().next(), Some("SourceFile,Make"));
}

#[test]
fn exclude_group_is_supported() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-x",
            "IFD0:Make",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(parsed[0].get("Make").is_none());
    assert!(parsed[0].get("Model").is_some());
}

#[test]
fn unknown_exclude_is_not_a_usage_error() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-x",
            "NoSuchTag",
            corpus("real/Canon_40D.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(parsed[0].get("Make").is_some());
}

#[test]
fn cr3_native_reader_without_fallback_reports_core_tags() {
    let output = Command::new(bin())
        .args([
            "-j",
            "--fallback=never",
            "-FileType",
            "-Make",
            "-ExifIFD:ExposureTime",
            "-GPSLatitude",
            corpus("real/canon-r6.cr3").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["FileType"], "CR3");
    assert_eq!(json[0]["Make"], "Canon");
    assert_eq!(json[0]["ExposureTime"], "1/100");
    assert!(json[0].get("Error").is_none());
}

#[test]
fn cr3_binary_extracts_thumbnail_and_preview() {
    let thumb = Command::new(bin())
        .args([
            "-b",
            "-ThumbnailImage",
            corpus("real/canon-r6.cr3").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(thumb.status.success());
    assert_eq!(thumb.stdout.len(), 12840);
    assert_eq!(&thumb.stdout[..2], &[0xFF, 0xD8]);

    let preview = Command::new(bin())
        .args([
            "-b",
            "-PreviewImage",
            corpus("real/canon-r6.cr3").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(preview.status.success());
    assert_eq!(preview.stdout.len(), 237320);
    assert_eq!(&preview.stdout[..2], &[0xFF, 0xD8]);
}

#[test]
fn cr3_native_canon_subtables_and_conversions() {
    let output = Command::new(bin())
        .args([
            "-j",
            "--fallback=never",
            "-Canon:AFAreaMode",
            "-Canon:TimeZone",
            "-Canon:FNumber",
            "-Canon:AFConfigTool",
            "-Canon:CompressorVersion",
            corpus("real/canon-r6.cr3").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["AFAreaMode"], "Flexizone Single");
    assert_eq!(json[0]["TimeZone"], "-08:00");
    assert_eq!(json[0]["FNumber"], 2.8);
    assert_eq!(json[0]["AFConfigTool"], "Case 1");
    assert_eq!(
        json[0]["CompressorVersion"],
        "CanonCR3_001/00.10.00/00.00.00"
    );
}

#[test]
fn heic_quicktime_metadata_is_native() {
    let output = Command::new(bin())
        .args([
            "-j",
            "--fallback=never",
            "-QuickTime:MajorBrand",
            "-QuickTime:ImageSpatialExtent",
            "-QuickTime:HEVCConfigurationVersion",
            "-QuickTime:BitDepthLuma",
            corpus("real/exif.heic").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json[0]["MajorBrand"],
        "High Efficiency Image Format HEVC still image (.HEIC)"
    );
    assert_eq!(json[0]["ImageSpatialExtent"], "4032x3024");
    assert_eq!(json[0]["HEVCConfigurationVersion"], 1);
    assert_eq!(json[0]["BitDepthLuma"], 8);
}
