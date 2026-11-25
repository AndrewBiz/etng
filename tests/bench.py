#!/usr/bin/env python3
"""Micro-benchmarks for etng vs exiftool.

Usage: python3 tests/bench.py [batch-dir]

`batch-dir` defaults to /tmp/etng-bench (10 000 JPEG copies). Results are
printed in markdown; copy them into BENCHMARKS.md with the machine info.
"""

import os
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ETNG = os.path.join(ROOT, "target", "release", "etng")
SINGLE = os.path.join(ROOT, "tests", "corpus", "synthetic", "base.jpg")
BATCH = sys.argv[1] if len(sys.argv) > 1 else "/tmp/etng-bench"


def measure(command, runs):
    times = []
    for _ in range(runs):
        started = time.perf_counter()
        subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        times.append(time.perf_counter() - started)
    return min(times), statistics.median(times)


def rss_mb(command):
    process = subprocess.Popen(
        command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    peak = 0
    while process.poll() is None:
        try:
            with open(f"/proc/{process.pid}/status", "r", encoding="utf-8") as status:
                for line in status:
                    if line.startswith("VmHWM:"):
                        peak = max(peak, int(line.split()[1]))
        except (FileNotFoundError, ProcessLookupError):
            break
        time.sleep(0.02)
    process.wait()
    return peak / 1024.0


def row(label, etng_cmd, exif_cmd, runs, rss=False):
    e_min, e_med = measure(etng_cmd, runs)
    x_min, x_med = measure(exif_cmd, runs)
    line = (
        f"| {label} | {e_med * 1000:.1f} | {x_med * 1000:.1f} | "
        f"{x_med / e_med:.1f}x |"
    )
    if rss:
        line += f" {rss_mb(etng_cmd):.0f} / {rss_mb(exif_cmd):.0f} |"
    print(line)


def main():
    if not os.path.exists(ETNG):
        sys.exit(f"missing {ETNG}; run `cargo build --release` first")
    print("| scenario | etng (ms, median) | exiftool (ms, median) | speedup | RSS MB etng/exiftool |")
    print("| --- | --- | --- | --- | --- |")
    row("startup (`-ver`)", [ETNG, "-ver"], ["exiftool", "-ver"], 20)
    row("single JPEG, `-j -G1`", [ETNG, "-j", "-G1", SINGLE],
        ["exiftool", "-j", "-G1", SINGLE], 10)
    if os.path.isdir(BATCH):
        count = len(os.listdir(BATCH))
        row(f"batch {count} JPEG, `-j`", [ETNG, "-j", BATCH], ["exiftool", "-j", BATCH], 3, rss=True)
    else:
        print(f"| batch skipped: {BATCH} missing | | | | |")


if __name__ == "__main__":
    main()
