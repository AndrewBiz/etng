# Benchmarks

Measured on the development machine (Arch Linux, kernel 7.3.5, 12 threads),
release build (`cargo build --release`), exiftool 13.55, corpus files described
in `tests/corpus/PROVENANCE.md`. Reproduce with:

```bash
cargo build --release
python3 tests/bench.py /tmp/etng-bench   # 10 000 copies of synthetic/base.jpg
```

## Before / after lazy tag tables

| scenario | before (ms) | after (ms) | exiftool (ms) | speedup after |
| --- | --- | --- | --- | --- |
| startup (`-ver`) | 0.9 | 0.9 | 66.4 | 71× |
| single JPEG, `-j -G1` | 34.8 | **8.1** | 101.7 | **12.6×** |
| batch 10 000 JPEG, `-j` | 288.8 | 273.9 | 48 766.9 | 178× |
| batch 10 000 JPEG, `-csv` | — | **486** | ~47 000 (2k: 9 378) | ~97× |
| peak RSS, batch (MB) | 22 | **12** | 27 | — |

«Before» — разбор всех 31 942 строк `tags.tsv` при первом обращении к таблицам.
«After» — индекс по group 1 и table при старте, разбор только реально
использованных групп, печатные конверсии разбираются по требованию.

CR3 (`canon-r6.cr3`, 336 КБ): нативный JSON `--fallback=never` — **1.6 мс**
против 196.6 мс у exiftool; `-b -PreviewImage` — **7.8 мс** против 187.7 мс
(24×). В `auto`-режиме JSON/CSV пока дополняются exiftool, поэтому там
доминирует делегирование.

CSV без выбора тегов делает два прохода (сбор колонок + вывод строк), поэтому
медленнее JSON, но всё равно на два порядка быстрее exiftool; RSS остаётся
18 МБ.

Notes:

- Одиночный файл теперь укладывается в цель <10 мс; остаток — парсинг самого
  JPEG и нескольких групп таблиц (IFD0/ExifIFD/GPS/IPTC/XMP).
- `-ver` и ошибки использования не трогают таблицы, поэтому старт
  субмиллисекундный.
- Пиковый RSS снимается из `/proc/<pid>/status:VmHWM`, пока процесс жив.
- Числа ориентировочные; перепроверяй `tests/bench.py` на целевой машине.
