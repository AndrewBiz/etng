# etng

ExifTool нового поколения: быстрый read-only CLI на Rust с JSON-выводом,
совместимым с `exiftool -j`.

## Возможности

- JPEG, TIFF, DNG, CR2, CR3, NEF, HEIC/HEIF; EXIF, IPTC, XMP, GPS, MakerNotes
  (Canon, Nikon, Sony, плоский Apple IFD), IFD1 thumbnail-теги.
- JSON-вывод в формате `exiftool -j`: порядок полей, `-G`/`-G1`, `-s`, `-n`,
  печатные конверсии, `ExifToolVersion` = версия совместимости (13.55).
- Выбор тегов `-TAG` / `-GROUP:TAG`, рекурсия `-r`, фильтр `-ext`,
  параллельная пакетная обработка с детерминированным порядком.
- CR3: нативный ISO-BMFF-ридер (EXIF/Canon/GPS, QuickTime/Track, THMB/PRVW);
  JSON по умолчанию дополняется exiftool до полного набора, `-b` работает без fallback.
- `-b`/`--binary`: сырые байты в stdout (миниатюры, превью CR2/CR3) и
  `"base64:<data>"` в JSON; скаляры без print-conversion.
- `-csv`: табличный вывод, `SourceFile` + колонки тегов; union колонок по
  алфавиту без выбора, порядок аргументов при `-TAG`.
- `-x TAG`/`-x GROUP:TAG`: исключение тегов из JSON, текста, `-b` и CSV.
- Гибридный fallback на системный `exiftool` (`auto|never|always`,
  переменная `ETNG_FALLBACK`), вызов без shell.
- Коды возврата: 0 — успех, 1 — предупреждения/частичные результаты,
  2 — ошибка использования.

## Установка

```bash
cargo build --release
install -m755 target/release/etng ~/.local/bin/etng
```

Зависимости: Rust 1.92+. Системный `exiftool` нужен только для fallback.

## Использование

```bash
etng -j photo.jpg                     # JSON как exiftool -j
etng -j -G1 -Make -Model ~/Photos     # группы и выбор тегов
etng -j -r -ext jpg -ext heic arch/   # рекурсивный пакетный режим
etng -j --fallback=never weird.pdf    # без делегирования
etng -b -ThumbnailImage photo.jpg > thumb.jpg
etng -j -b -ThumbnailImage photo.jpg  # base64 в JSON
etng -csv -r -ext jpg ~/Photos > all.csv
etng -csv -Make -Model -x Model photo.jpg
etng -j -x MakerNotes photo.jpg       # исключить шумный тег
etng -ver                             # версия совместимости
```

Текстовый режим (без `-j`) печатает `Tag : value` по файлу.

## Разработка

```bash
cargo test --workspace                 # unit + CLI + streaming + mutations
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cargo xtask generate-tags              # пересобрать таблицы из снимка -listx
cargo xtask snapshots                  # эталонные JSON-снимки exiftool
cargo xtask compare                    # дифф против эталонов
python3 tests/compare_corpus.py         # тот же дифф, отчёт по файлам
python3 tests/bench.py                  # замеры против exiftool
```

Структура:

- `crates/etng-core` — модель тегов, парсеры, потоковый JSON-писатель.
- `crates/etng-cli` — бинарник `etng`: аргументы, обход, fallback.
- `crates/etng-tags` — таблицы тегов из `exiftool -listx` (31 942 тега).
- `xtask` — кодогенерация, снимки, компаратор.

Документация: `docs/tag-tables.md` (обновление таблиц),
`docs/divergences.md` (осознанные расхождения с exiftool 13.55),
`BENCHMARKS.md` (замеры).
