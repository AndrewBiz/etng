#!/usr/bin/env python3
"""Run a read/write audit of etng on a copy of the local photo corpus.

Original files are never touched.  The audit:
  * copies the corpus to a temporary directory;
  * compares etng and exiftool JSON before modifying anything;
  * runs phls, phrename and phfixdto through the exiftool shim;
  * compares etng/exiftool again and runs exiftool -validate;
  * measures every stage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from collections import Counter
from pathlib import Path

EXIFTOOL = "/usr/bin/vendor_perl/exiftool"
DEFAULT_CORPUS = Path.home() / "Pictures" / "test_corpus"
DEFAULT_ETNG = Path.home() / "work" / "etng" / "target" / "release" / "etng"
DEFAULT_PHTOOLS = Path.home() / "work" / "phtools"
SHIM = Path.home() / "work" / "etng" / "tools" / "exiftool-shim" / "exiftool"

IGNORED_KEYS = {
    "SourceFile",
    "System:FileName",
    "System:Directory",
    "System:FileAccessDate",
    "System:FileInodeChangeDate",
    "File:FileName",
    "File:Directory",
    "File:FileAccessDate",
    "File:FileInodeChangeDate",
}


def run(cmd, *, cwd=None, env=None, input_text=None):
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def read_json(binary: Path, path: Path, extra=()):
    command = [str(binary), "-j", "-G1", *extra, str(path)]
    if binary.name == "etng":
        command.insert(1, "--fallback=never")
    result = run(command)
    if result.returncode != 0 and not result.stdout.strip():
        return None, result.stderr.strip()
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as err:
        return None, f"invalid JSON: {err}"
    return (data[0] if data else {}), None


def normalized(tags):
    return {key: value for key, value in tags.items() if key not in IGNORED_KEYS}


def compare(before_etng, before_exif, etng_bin, exiftool_bin):
    files = []
    common_diffs = Counter()
    missing = Counter()
    extra = Counter()
    total_missing = 0
    total_extra = 0
    total_value_diff = 0
    for path in before_etng:
        etng, etng_err = before_etng[path]
        exif, exif_err = before_exif[path]
        if etng is None or exif is None:
            files.append(
                {
                    "file": path.name,
                    "error": etng_err or exif_err or "read failed",
                }
            )
            continue
        etng = normalized(etng)
        exif = normalized(exif)
        etng_keys = set(etng)
        exif_keys = set(exif)
        file_missing = sorted(exif_keys - etng_keys)
        file_extra = sorted(etng_keys - exif_keys)
        value_diffs = []
        for key in sorted(etng_keys & exif_keys):
            if etng[key] != exif[key]:
                value_diffs.append(
                    {"tag": key, "etng": etng[key], "exiftool": exif[key]}
                )
        total_missing += len(file_missing)
        total_extra += len(file_extra)
        total_value_diff += len(value_diffs)
        missing.update(file_missing)
        extra.update(file_extra)
        common_diffs.update(item["tag"] for item in value_diffs)
        files.append(
            {
                "file": path.name,
                "etng_tags": len(etng_keys),
                "exiftool_tags": len(exif_keys),
                "missing": file_missing,
                "extra": file_extra,
                "value_diffs": value_diffs,
            }
        )
    return {
        "files": files,
        "total_missing": total_missing,
        "total_extra": total_extra,
        "total_value_diff": total_value_diff,
        "top_missing": missing.most_common(15),
        "top_extra": extra.most_common(15),
        "top_value_diffs": common_diffs.most_common(15),
    }


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def phtools_commands(phtools: Path, shim_dir: Path, etng: Path, cwd: Path, log: Path):
    env = dict(os.environ)
    env["BUNDLE_GEMFILE"] = str(phtools / "Gemfile")
    env["PATH"] = f"{shim_dir}:{phtools / 'exe'}:{env.get('PATH', '')}"
    env["ETNG_BIN"] = str(etng)
    env["ETNG_SHIM_LOG"] = str(log)
    ruby = ["bundle", "exec", "ruby", "-I", str(phtools / "lib")]
    return env, ruby


def pipe(cmd1, cmd2, *, env, cwd):
    p1 = subprocess.Popen(
        cmd1, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    )
    p2 = subprocess.Popen(
        cmd2,
        cwd=cwd,
        env=env,
        stdin=p1.stdout,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if p1.stdout is not None:
        p1.stdout.close()
    out2, err2 = p2.communicate()
    _, err1 = p1.communicate()
    return p1.returncode, p2.returncode, out2, err1 + err2


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--etng", type=Path, default=DEFAULT_ETNG)
    parser.add_argument("--phtools", type=Path, default=DEFAULT_PHTOOLS)
    parser.add_argument("--limit", type=int, default=0, help="0 = all files")
    parser.add_argument("--report", type=Path)
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()

    if not args.etng.exists():
        print(f"etng not found: {args.etng}", flush=True)
        return 2
    if not args.phtools.exists():
        print(f"phtools not found: {args.phtools}", flush=True)
        return 2

    started = time.perf_counter()
    timings = {}
    files = sorted(
        path for path in args.corpus.iterdir() if path.is_file() and not path.name.startswith(".")
    )
    if args.limit:
        files = files[: args.limit]

    root = Path(tempfile.mkdtemp(prefix="etng-audit-"))
    copy_dir = root / "copy"
    rename_dir = root / "rename"
    shim_dir = root / "bin"
    copy_dir.mkdir()
    rename_dir.mkdir()
    shim_dir.mkdir()
    shutil.copy2(SHIM, shim_dir / "exiftool")
    (shim_dir / "exiftool").chmod(0o755)
    shim_log = root / "shim.log"

    print(f"copying {len(files)} files...", flush=True)
    stage = time.perf_counter()
    for path in files:
        shutil.copy2(path, copy_dir / path.name)
        shutil.copy2(path, rename_dir / path.name)
    timings["copy"] = round(time.perf_counter() - stage, 3)
    print(f"copied in {timings['copy']}s", flush=True)

    exiftool = Path(EXIFTOOL)
    print("reading with etng...", flush=True)
    stage = time.perf_counter()
    before_etng = {path: read_json(args.etng, path) for path in files}
    timings["etng_read_before"] = round(time.perf_counter() - stage, 3)
    print(f"etng read in {timings['etng_read_before']}s", flush=True)
    print("reading with exiftool...", flush=True)
    stage = time.perf_counter()
    before_exif = {copy_dir / path.name: read_json(exiftool, copy_dir / path.name) for path in files}
    timings["exiftool_read_before"] = round(time.perf_counter() - stage, 3)
    print(f"exiftool read in {timings['exiftool_read_before']}s", flush=True)
    # map exiftool results back to original paths
    before_exif = {path: before_exif[copy_dir / path.name] for path in files}
    timings["read_compare"] = 0.0

    env, ruby = phtools_commands(args.phtools, shim_dir, args.etng, copy_dir, shim_log)

    print("running phls...", flush=True)
    stage = time.perf_counter()
    result = run(ruby + [str(args.phtools / "exe" / "phls"), "."], cwd=copy_dir, env=env)
    timings["phls"] = round(time.perf_counter() - stage, 3)
    print(f"phls in {timings['phls']}s", flush=True)
    phls_ok = result.returncode == 0

    print("running phrename...", flush=True)
    stage = time.perf_counter()
    rename1 = pipe(
        ruby + [str(args.phtools / "exe" / "phls"), "."],
        ruby + [str(args.phtools / "exe" / "phrename"), "-a", "anb"],
        env=env,
        cwd=rename_dir,
    )
    timings["phrename"] = round(time.perf_counter() - stage, 3)
    print(f"phrename in {timings['phrename']}s", flush=True)

    renamed = sorted(rename_dir.iterdir()) if rename_dir.exists() else []
    hashes_before_second = {path.name: sha256(path) for path in renamed if path.is_file()}
    stage = time.perf_counter()
    rename2 = pipe(
        ruby + [str(args.phtools / "exe" / "phls"), "."],
        ruby + [str(args.phtools / "exe" / "phrename"), "-a", "anb"],
        env=env,
        cwd=rename_dir,
    )
    timings["phrename_second"] = round(time.perf_counter() - stage, 3)
    hashes_after_second = {path.name: sha256(path) for path in renamed if path.is_file()}
    rename_idempotent = hashes_before_second == hashes_after_second

    renamed_files = sorted(rename_dir.iterdir())
    print("running phfixdto...", flush=True)
    stage = time.perf_counter()
    fix1 = pipe(
        ruby + [str(args.phtools / "exe" / "phls"), "."],
        ruby + [str(args.phtools / "exe" / "phfixdto")],
        env=env,
        cwd=rename_dir,
    )
    timings["phfixdto"] = round(time.perf_counter() - stage, 3)
    print(f"phfixdto in {timings['phfixdto']}s", flush=True)

    stage = time.perf_counter()
    fix_hashes = {path.name: sha256(path) for path in renamed_files if path.is_file()}
    fix2 = pipe(
        ruby + [str(args.phtools / "exe" / "phls"), "."],
        ruby + [str(args.phtools / "exe" / "phfixdto")],
        env=env,
        cwd=rename_dir,
    )
    timings["phfixdto_second"] = round(time.perf_counter() - stage, 3)
    fix_hashes_after = {path.name: sha256(path) for path in renamed_files if path.is_file()}
    fix_idempotent = fix_hashes == fix_hashes_after

    stage = time.perf_counter()
    after_etng = {path: read_json(args.etng, path) for path in renamed_files}
    timings["etng_read_after"] = round(time.perf_counter() - stage, 3)
    stage = time.perf_counter()
    after_exif = {path: read_json(exiftool, path) for path in renamed_files}
    timings["exiftool_read_after"] = round(time.perf_counter() - stage, 3)

    print("validating...", flush=True)
    stage = time.perf_counter()
    validation_before = {}
    for path in files:
        result = run([str(exiftool), "-validate", "-warning", "-error", str(copy_dir / path.name)])
        validation_before[path.name] = result.stdout.strip()
    validation_after = {}
    for path in renamed_files:
        result = run([str(exiftool), "-validate", "-warning", "-error", str(path)])
        validation_after[path.name] = result.stdout.strip()
    timings["validate"] = round(time.perf_counter() - stage, 3)

    before_map = {copy_dir / path.name: before_exif[path] for path in files}
    before_etng_copy = {copy_dir / path.name: before_etng[path] for path in files}
    stage = time.perf_counter()
    read_report = compare(before_etng_copy, before_map, args.etng, exiftool)
    after_report = compare(after_etng, after_exif, args.etng, exiftool)
    timings["read_compare"] = round(time.perf_counter() - stage, 3)

    # Warnings are pre-existing in many camera files; only new errors count.
    def has_error(text: str) -> bool:
        return any(line.strip().startswith("Error") for line in text.splitlines())

    clean = all(not has_error(text) for text in validation_after.values())
    new_warnings = {
        name: text
        for name, text in validation_after.items()
        if text != validation_before.get(name) and "Warning" in text
    }
    total = round(time.perf_counter() - started, 3)
    report = {
        "corpus": str(args.corpus),
        "files": len(files),
        "timings_s": {**timings, "total": total},
        "phls_ok": phls_ok,
        "phrename": {
            "returncodes": rename1[:2],
            "output": rename1[2],
            "stderr": rename1[3],
            "idempotent": rename_idempotent,
            "second_returncodes": rename2[:2],
        },
        "phfixdto": {
            "returncodes": fix1[:2],
            "stderr": fix1[3],
            "idempotent": fix_idempotent,
            "second_returncodes": fix2[:2],
        },
        "validate_ok": clean,
        "validation_before": validation_before,
        "validation_after": validation_after,
        "new_warnings": new_warnings,
        "read_before": read_report,
        "read_after": after_report,
    }

    output = args.report or root / "report.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"report: {output}")
    print(f"files: {len(files)}")
    print("timings:", json.dumps(report["timings_s"], ensure_ascii=False))
    print(
        "read before: missing={} extra={} value_diff={}".format(
            read_report["total_missing"],
            read_report["total_extra"],
            read_report["total_value_diff"],
        )
    )
    print(
        "read after:  missing={} extra={} value_diff={}".format(
            after_report["total_missing"],
            after_report["total_extra"],
            after_report["total_value_diff"],
        )
    )
    print(
        "phtools: phls={} phrename={} phfixdto={} validate={}".format(
            phls_ok,
            rename1[0] == 0 and rename1[1] == 0,
            fix1[0] == 0 and fix1[1] == 0,
            clean,
        )
    )
    if args.keep:
        print(f"workdir: {root}")
    else:
        shutil.rmtree(root, ignore_errors=True)
    return 0 if clean and phls_ok and rename1[0] == 0 and rename1[1] == 0 and fix1[0] == 0 and fix1[1] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
