# Benchmarks

Measured on the development machine (Arch Linux, kernel 7.3.5, 12 threads),
release build (`cargo build --release`), exiftool 13.55, corpus files described
in `tests/corpus/PROVENANCE.md`. Reproduce with:

```bash
cargo build --release
python3 tests/bench.py /tmp/etng-bench   # 10 000 copies of synthetic/base.jpg
```

| scenario | etng (ms, median) | exiftool (ms, median) | speedup | peak RSS, MB (etng / exiftool) |
| --- | --- | --- | --- | --- |
| startup (`-ver`) | 0.9 | 66.6 | 70.7× | — |
| single JPEG, `-j -G1` | 34.8 | 101.9 | 2.9× | — |
| batch 10 000 JPEG, `-j` | 288.8 | 48 836.2 | 169.1× | 22 / 27 |

Notes:

- The single-file number includes the one-time lazy parse of the embedded tag
  table (31 942 tags, 4.5 MB TSV). Batch mode amortises it, which is why the
  10k-file case is two orders of magnitude faster.
- `-ver` and argument errors never touch the tag table, hence sub-millisecond
  startup.
- Peak RSS is sampled from `/proc/<pid>/status:VmHWM` while the process runs.
- Numbers are indicative, not a guarantee; re-run `tests/bench.py` on the
  target machine.
