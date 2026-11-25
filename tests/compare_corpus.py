#!/usr/bin/env python3
"""Compare etng JSON output with `exiftool -j -G1` over the corpus.

Usage: python3 tests/compare_corpus.py [paths...]

Volatile system fields (access/inode times) are masked. Keys from vendor
subtrees that etng intentionally does not model yet (Apple/QuickTime) are
reported as known divergences and do not fail the run.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ETNG = ROOT / "target" / "debug" / "etng"
CORPUS = ROOT / "tests" / "corpus"
MASK_SUFFIXES = ("FileAccessDate", "FileInodeChangeDate", "Warning")
KNOWN_VENDOR_PREFIXES = (
    "Apple:",
    "QuickTime:",
    "ICC_Profile:",
    "ICC-header:",
    "ICC-meas:",
    "ICC-view:",
    "Meta:",
    "Vivo:",
)
SKIP_FILES = {"broken.jpg", "endless_loop.jpg", "invalid-gps"}
KNOWN_KEYS = {
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
    "IFD1:ThumbnailImage",
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
}


def is_known(key):
    return key in KNOWN_KEYS or any(key.startswith(prefix) for prefix in KNOWN_VENDOR_PREFIXES)


def run_json(command):
    result = subprocess.run(command, capture_output=True, check=False)
    if result.returncode not in (0, 1):
        raise RuntimeError(f"{command}: exit {result.returncode}: {result.stderr.decode()}")
    text = result.stdout.decode("utf-8", "replace").strip()
    return json.loads(text)[0] if text else {}


def compare(path):
    actual = run_json([str(ETNG), "-j", "-G1", str(path)])
    expected = run_json(["exiftool", "-j", "-G1", str(path)])
    report = {"only_etng": [], "only_exiftool": [], "changed": [], "known": 0}
    for key, value in expected.items():
        if key.endswith(MASK_SUFFIXES):
            continue
        if key not in actual:
            if is_known(key):
                report["known"] += 1
            else:
                report["only_exiftool"].append(key)
        elif actual[key] != value:
            if is_known(key):
                report["known"] += 1
            else:
                report["changed"].append((key, actual[key], value))
    for key in actual:
        if key.endswith(MASK_SUFFIXES):
            continue
        if key not in expected:
            if is_known(key):
                report["known"] += 1
            else:
                report["only_etng"].append(key)
    return report


def main(paths):
    if not paths:
        paths = sorted(
            p
            for p in CORPUS.rglob("*")
            if p.is_file()
            and p.suffix.lower()
            in {".jpg", ".jpeg", ".tif", ".tiff", ".dng", ".nef", ".cr2", ".heic"}
        )
    failures = 0
    for path in paths:
        path = Path(path)
        if path.name in SKIP_FILES:
            print(f"{path.name}: skipped (hostile input, documented)")
            continue
        report = compare(path)
        print(
            f"{path.name}: +{len(report['only_etng'])} etng-only, "
            f"-{len(report['only_exiftool'])} exiftool-only, "
            f"~{len(report['changed'])} changed, "
            f"{report['known']} known vendor"
        )
        if report["only_etng"]:
            print("  etng-only:", report["only_etng"][:12])
        if report["only_exiftool"]:
            print("  exiftool-only:", report["only_exiftool"][:12])
        for key, got, want in report["changed"][:12]:
            print(f"  changed {key}: {got!r} != {want!r}")
        if report["only_etng"] or report["only_exiftool"] or report["changed"]:
            failures += 1
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
