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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 4393);
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json[0]["ThumbnailImage"],
        "(Binary data 4393 bytes, use -b option to extract)"
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("SourceFile,Orientation,FileSize"));
    let row = lines.next().unwrap();
    assert!(row.ends_with(",1,8893"), "{row}");
}

#[test]
fn exclude_removes_fields_and_columns() {
    let file = corpus("synthetic/thumb.jpg");
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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
            corpus("synthetic/thumb.jpg").to_str().unwrap(),
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

#[test]
fn icc_profile_is_native_for_jpeg_and_heic() {
    for file in ["real/exif.jpg", "real/exif.heic"] {
        let output = Command::new(bin())
            .args([
                "-j",
                "-G1",
                "--fallback=never",
                corpus(file).to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            !output.stdout.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let object = json[0].as_object().unwrap();
        let icc: Vec<_> = object.keys().filter(|key| key.starts_with("ICC")).collect();
        assert_eq!(icc.len(), 26, "{file}");
        assert_eq!(object["ICC_Profile:ProfileDescription"], "Display P3");
        assert!(
            object["ICC_Profile:MediaWhitePoint"]
                .as_str()
                .is_some_and(|value| value.split(' ').count() == 3)
        );
    }
}

#[test]
fn png_is_native_with_exif_and_png_chunks() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "--fallback=never",
            corpus("synthetic/base.png").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let object = json[0].as_object().unwrap();
    assert_eq!(object["PNG:ImageWidth"], 64);
    assert_eq!(object["PNG:ImageHeight"], 48);
    assert_eq!(object["PNG:BitDepth"], 8);
    assert_eq!(object["PNG:ColorType"], "RGB");
    assert_eq!(object["IFD0:ImageWidth"], 64);
}

fn strip_png_chunk(data: &[u8], kind: &[u8; 4]) -> Vec<u8> {
    let mut out = data[..8].to_vec();
    let mut pos = 8;
    while pos + 12 <= data.len() {
        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let end = pos + 12 + length;
        if &data[pos + 4..pos + 8] != kind {
            out.extend_from_slice(&data[pos..end]);
        }
        pos = end;
    }
    out
}

#[test]
fn composite_optical_and_subsec_tags_match_exiftool() {
    let output = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-Composite:ScaleFactor35efl",
            "-Composite:CircleOfConfusion",
            "-Composite:FOV",
            "-Composite:HyperfocalDistance",
            "-Composite:GPSAltitude",
            "-Composite:SubSecDateTimeOriginal",
            corpus("real/exif.jpg").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["Composite:ScaleFactor35efl"], 2.7);
    assert_eq!(json[0]["Composite:CircleOfConfusion"], "0.011 mm");
    assert_eq!(json[0]["Composite:FOV"], "76.1 deg");
    assert_eq!(json[0]["Composite:HyperfocalDistance"], "3.79 m");
    assert_eq!(json[0]["Composite:GPSAltitude"], "0 m Above Sea Level");
    assert_eq!(
        json[0]["Composite:SubSecDateTimeOriginal"],
        "2023:07:09 20:36:33.616+08:00"
    );
}

#[test]
fn writes_png_dates_into_existing_exif() {
    let dir = temp_dir("write-png-exif");
    let path = dir.join("copy.png");
    fs::copy(corpus("synthetic/base.png"), &path).unwrap();
    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            "-CreateDate=2024:05:06 07:08:09",
            "-ModifyDate=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-DateTimeOriginal",
            "-CreateDate",
            "-ModifyDate",
            "-PNG:ImageWidth",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["ExifIFD:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IFD0:ModifyDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["PNG:ImageWidth"], 64);
}

#[test]
fn writes_png_dates_without_exif() {
    let dir = temp_dir("write-png-noexif");
    let path = dir.join("copy.png");
    let source = fs::read(corpus("synthetic/base.png")).unwrap();
    fs::write(&path, strip_png_chunk(&source, b"eXIf")).unwrap();
    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            "-CreateDate=2024:05:06 07:08:09",
            "-ModifyDate=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-DateTimeOriginal",
            "-CreateDate",
            "-ModifyDate",
            "-PNG:ImageWidth",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["ExifIFD:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IFD0:ModifyDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["PNG:ImageWidth"], 64);
}

#[test]
fn writes_datetime_original_in_place() {
    let dir = temp_dir("write-dto");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("copy.jpg");
    fs::copy(corpus("real/exif.jpg"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args(["-j", "-G1", "-DateTimeOriginal", path.to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
}

#[test]
fn writes_file_modify_date() {
    let dir = temp_dir("write-fmd");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("copy.jpg");
    fs::copy(corpus("real/exif.jpg"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-FileModifyDate=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args(["-j", "-G1", "-FileModifyDate", path.to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert!(
        json[0]["System:FileModifyDate"]
            .as_str()
            .is_some_and(|value| value.starts_with("2024:05:06 07:08:09"))
    );

    let text = Command::new(bin())
        .args(["-s", "-FileModifyDate", path.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.lines()
            .any(|line| line.starts_with("FileModifyDate") && line.contains("2024:05:06 07:08:09")),
        "{text}"
    );
}

#[test]
fn write_missing_date_tag_inserts_without_corrupting_file() {
    let dir = temp_dir("write-missing");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("copy.jpg");
    fs::copy(corpus("real/tif.tif"), &path).unwrap();
    let before = fs::read(&path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(fs::read(&path).unwrap(), before);
    let read = Command::new(bin())
        .args(["-j", "-G1", "-DateTimeOriginal", path.to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["IFD0:DateTimeOriginal"], "2024:05:06 07:08:09");
}

#[test]
fn inserts_missing_datetime_original() {
    let dir = temp_dir("insert-dto");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("missing.jpg");
    fs::copy(corpus("phtools/tags/tag_dto3.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-DateTimeOriginal",
            "-Make",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IFD0:Make"], "SONY");
}

#[test]
fn inserts_missing_modify_date() {
    let dir = temp_dir("insert-modify");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("missing-modify.jpg");
    fs::copy(corpus("synthetic/base.jpg"), &path).unwrap();

    let output = Command::new(bin())
        .args(["-ModifyDate=2024:05:06 07:08:09", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args(["-j", "-G1", "-ModifyDate", path.to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["IFD0:ModifyDate"], "2024:05:06 07:08:09");
}

#[test]
fn all_dates_inserts_missing_exif_dates() {
    let dir = temp_dir("insert-all");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("all-dates.jpg");
    fs::copy(corpus("phtools/tags/tag_dto3.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args(["-AllDates=2024:05:06 07:08:09", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-DateTimeOriginal",
            "-CreateDate",
            "-ModifyDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["ExifIFD:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IFD0:ModifyDate"], "2024:05:06 07:08:09");
}

#[test]
fn writes_xmp_and_iptc_datetimeoriginal() {
    let dir = temp_dir("sidecar-dto");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dates.jpg");
    fs::copy(corpus("phtools/dates/1exif_datetimeoriginal.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-photoshop:DateCreated",
            "-IPTC:DateCreated",
            "-IPTC:TimeCreated",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-photoshop:DateCreated"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IPTC:DateCreated"], "2024:05:06");
    assert!(
        json[0]["IPTC:TimeCreated"]
            .as_str()
            .unwrap()
            .starts_with("07:08:09")
    );
}

#[test]
fn writes_xmp_and_iptc_createdate() {
    let dir = temp_dir("sidecar-cd");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dates.jpg");
    fs::copy(corpus("phtools/dates/1exif_datetimeoriginal.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args(["-CreateDate=2024:05:06 07:08:09", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-xmp:CreateDate",
            "-IPTC:DigitalCreationDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-xmp:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IPTC:DigitalCreationDate"], "2024:05:06");
}

#[test]
fn creates_xmp_and_iptc_dates_when_absent() {
    let dir = temp_dir("sidecar-create");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("no-sidecars.jpg");
    fs::copy(corpus("phtools/dates/7filemodifydate.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args(["-AllDates=2024:05:06 07:08:09", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-photoshop:DateCreated",
            "-XMP-xmp:CreateDate",
            "-XMP-xmp:ModifyDate",
            "-IPTC:DateCreated",
            "-IPTC:TimeCreated",
            "-IPTC:DigitalCreationDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-photoshop:DateCreated"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["XMP-xmp:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["XMP-xmp:ModifyDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IPTC:DateCreated"], "2024:05:06");
    assert_eq!(json[0]["IPTC:TimeCreated"], "07:08:09+00:00");
    assert_eq!(json[0]["IPTC:DigitalCreationDate"], "2024:05:06");
}

#[test]
fn writes_mwg_text_tags() {
    let dir = temp_dir("mwg-text");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tags.jpg");
    fs::copy(corpus("phtools/dates/7filemodifydate.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-City=Moscow",
            "-Country=Russia",
            "-State=Moscow",
            "-Location=Fenino",
            "-Keywords=test",
            "-Creator=ANB",
            "-Copyright=(c) ANB",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-photoshop:City",
            "-XMP-photoshop:Country",
            "-XMP-photoshop:State",
            "-XMP-iptcCore:Location",
            "-XMP-dc:Subject",
            "-XMP-dc:Creator",
            "-XMP-dc:Rights",
            "-IPTC:City",
            "-IPTC:Country-PrimaryLocationName",
            "-IPTC:Province-State",
            "-IPTC:Sub-location",
            "-IPTC:Keywords",
            "-IPTC:By-line",
            "-IPTC:CopyrightNotice",
            "-IFD0:Artist",
            "-IFD0:Copyright",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-photoshop:City"], "Moscow");
    assert_eq!(json[0]["XMP-photoshop:Country"], "Russia");
    assert_eq!(json[0]["XMP-photoshop:State"], "Moscow");
    assert_eq!(json[0]["XMP-iptcCore:Location"], "Fenino");
    assert_eq!(json[0]["XMP-dc:Subject"], "test");
    assert_eq!(json[0]["XMP-dc:Creator"], "ANB");
    assert_eq!(json[0]["XMP-dc:Rights"], "(c) ANB");
    assert_eq!(json[0]["IPTC:City"], "Moscow");
    assert_eq!(json[0]["IPTC:Country-PrimaryLocationName"], "Russia");
    assert_eq!(json[0]["IPTC:Province-State"], "Moscow");
    assert_eq!(json[0]["IPTC:Sub-location"], "Fenino");
    assert_eq!(json[0]["IPTC:Keywords"], "test");
    assert_eq!(json[0]["IPTC:By-line"], "ANB");
    assert_eq!(json[0]["IPTC:CopyrightNotice"], "(c) ANB");
    assert_eq!(json[0]["IFD0:Artist"], "ANB");
    assert_eq!(json[0]["IFD0:Copyright"], "(c) ANB");
}

#[test]
fn writes_multi_value_keywords_and_creator() {
    let dir = temp_dir("mwg-multi");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("multi.jpg");
    fs::copy(corpus("phtools/dates/7filemodifydate.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-Keywords=alpha",
            "-Keywords=beta",
            "-Creator=Alice",
            "-Creator=Bob",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-dc:Subject",
            "-IPTC:Keywords",
            "-XMP-dc:Creator",
            "-IPTC:By-line",
            "-IFD0:Artist",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-dc:Subject"], "alpha, beta");
    assert_eq!(json[0]["IPTC:Keywords"], "alpha, beta");
    assert_eq!(json[0]["XMP-dc:Creator"], "Alice, Bob");
    assert_eq!(json[0]["IPTC:By-line"], "Alice, Bob");
    assert_eq!(json[0]["IFD0:Artist"], "Alice; Bob");
}

#[test]
fn writes_heic_exif_date() {
    let dir = temp_dir("heic-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("photo.heic");
    fs::copy(corpus("real/exif.heic"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args(["-j", "-G1", "-DateTimeOriginal", path.to_str().unwrap()])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
}

#[test]
fn writes_cr3_exif_and_quicktime_dates() {
    let dir = temp_dir("cr3-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("photo.cr3");
    fs::copy(corpus("real/canon-r6.cr3"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-DateTimeOriginal=2024:05:06 07:08:09",
            "-CreateDate=2024:05:06 07:08:09",
            "-ModifyDate=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-ExifIFD:DateTimeOriginal",
            "-ExifIFD:CreateDate",
            "-IFD0:ModifyDate",
            "-QuickTime:CreateDate",
            "-QuickTime:ModifyDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["ExifIFD:DateTimeOriginal"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["ExifIFD:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["IFD0:ModifyDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["QuickTime:CreateDate"], "2024:05:06 07:08:09+03:00");
    assert_eq!(json[0]["QuickTime:ModifyDate"], "2024:05:06 07:08:09+03:00");
}

fn minimal_mov() -> Vec<u8> {
    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32 + 8).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }
    let mut ftyp = b"qt  ".to_vec();
    ftyp.extend_from_slice(&0u32.to_be_bytes());
    ftyp.extend_from_slice(b"qt  ");
    let mut mvhd = vec![0u8; 100];
    mvhd[0] = 0;
    mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd[16..20].copy_from_slice(&1000u32.to_be_bytes());
    let moov = atom(b"moov", &atom(b"mvhd", &mvhd));
    let mut out = atom(b"ftyp", &ftyp);
    out.extend_from_slice(&moov);
    out
}

#[test]
fn writes_mov_header_dates() {
    use std::io::Write;
    let dir = temp_dir("mov-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clip.mov");
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(&minimal_mov()).unwrap();

    let output = Command::new(bin())
        .args([
            "-CreateDate=2024:05:06 07:08:09",
            "-ModifyDate=2024:05:06 07:08:09",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-CreateDate",
            "-ModifyDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["QuickTime:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["QuickTime:ModifyDate"], "2024:05:06 07:08:09");
}

fn minimal_mov_with_apple_key(created: &str) -> Vec<u8> {
    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32 + 8).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }
    let mut ftyp = b"qt  ".to_vec();
    ftyp.extend_from_slice(&0u32.to_be_bytes());
    ftyp.extend_from_slice(b"qt  ");
    let mut mvhd = vec![0u8; 100];
    mvhd[0] = 0;
    mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd[16..20].copy_from_slice(&1000u32.to_be_bytes());

    let key = b"com.apple.quicktime.creationdate";
    let mut key_entry = Vec::new();
    key_entry.extend_from_slice(&(8 + key.len() as u32).to_be_bytes());
    key_entry.extend_from_slice(b"mdta");
    key_entry.extend_from_slice(key);
    let mut keys = vec![0u8; 4];
    keys.extend_from_slice(&1u32.to_be_bytes());
    keys.extend_from_slice(&key_entry);

    let mut data = vec![1u32, 0u32]
        .into_iter()
        .flat_map(u32::to_be_bytes)
        .collect::<Vec<_>>();
    data.extend_from_slice(created.as_bytes());
    let item_kind = 1u32.to_be_bytes();
    let item = atom(&item_kind, &atom(b"data", &data));
    let mut meta_payload = atom(b"hdlr", &[0; 24]);
    meta_payload.extend_from_slice(&atom(b"keys", &keys));
    meta_payload.extend_from_slice(&atom(b"ilst", &item));
    let meta = atom(b"meta", &meta_payload);

    let mut moov_payload = atom(b"mvhd", &mvhd);
    moov_payload.extend_from_slice(&meta);
    let moov = atom(b"moov", &moov_payload);
    let mut out = atom(b"ftyp", &ftyp);
    out.extend_from_slice(&moov);
    out
}

#[test]
fn writes_mov_apple_creation_date() {
    use std::io::Write;
    let dir = temp_dir("mov-apple-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clip.mov");
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(&minimal_mov_with_apple_key("2001-01-01T01:01:01+0300"))
        .unwrap();

    let output = Command::new(bin())
        .args(["-CreateDate=2024:05:06 07:08:09", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-CreateDate",
            "-Keys:CreationDate",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["QuickTime:CreateDate"], "2024:05:06 07:08:09");
    assert_eq!(json[0]["Keys:CreationDate"], "2024:05:06 07:08:09+03:00");
}

#[test]
fn writes_phtools_extra_tags() {
    let dir = temp_dir("phtools-extra");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("extra.jpg");
    fs::copy(corpus("phtools/dates/1exif_datetimeoriginal.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-CountryCode=RU",
            "-WorldRegion=Europe",
            "-CodedCharacterSet=UTF8",
            "-ImageUniqueID=abc123",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-iptcExt:LocationShownCountryCode",
            "-XMP-iptcExt:LocationShownWorldRegion",
            "-ExifIFD:ImageUniqueID",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-iptcExt:LocationShownCountryCode"], "RU");
    assert_eq!(json[0]["XMP-iptcExt:LocationShownWorldRegion"], "Europe");
    assert_eq!(json[0]["ExifIFD:ImageUniqueID"], "abc123");
}

#[test]
fn writes_gps_tags() {
    let dir = temp_dir("gps-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("gps.jpg");
    fs::copy(corpus("real/tif.tif"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-GPSLatitude=55.75",
            "-GPSLatitudeRef=N",
            "-GPSLongitude=37.6173",
            "-GPSLongitudeRef=E",
            "-GPSAltitude=150.5",
            "-GPSAltitudeRef=0",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-Composite:GPSLatitude",
            "-Composite:GPSLongitude",
            "-GPS:GPSAltitude",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert!(
        json[0]["Composite:GPSLatitude"]
            .as_str()
            .unwrap()
            .starts_with("55 deg 45")
    );
    assert!(
        json[0]["Composite:GPSLongitude"]
            .as_str()
            .unwrap()
            .starts_with("37 deg 37")
    );
    assert!(
        json[0]["GPS:GPSAltitude"]
            .as_str()
            .unwrap()
            .starts_with("150.5 m")
    );
}

#[test]
fn writes_collections() {
    let dir = temp_dir("collections-write");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("collections.jpg");
    fs::copy(corpus("phtools/dates/7filemodifydate.JPG"), &path).unwrap();

    let output = Command::new(bin())
        .args([
            "-Collections=Name 1|uri1",
            "-Collections=Name 2|uri2",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = Command::new(bin())
        .args([
            "-j",
            "-G1",
            "-XMP-mwg-coll:CollectionName",
            "-XMP-mwg-coll:CollectionURI",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(json[0]["XMP-mwg-coll:CollectionName"], "Name 1");
    assert_eq!(json[0]["XMP-mwg-coll:CollectionURI"], "uri1");
}

#[test]
fn lists_tags() {
    let output = Command::new(bin()).arg("-list").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("Available tags:"), "{text}");
    assert!(text.contains("DateTimeOriginal"), "{text}");

    let output = Command::new(bin()).arg("-listw").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("Writable tags:"), "{text}");
    assert!(text.contains("DateTimeOriginal"), "{text}");
}
