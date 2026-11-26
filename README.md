# etng

[![CI](https://github.com/AndrewBiz/etng/actions/workflows/ci.yml/badge.svg)](https://github.com/AndrewBiz/etng/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/AndrewBiz/etng?display_name=tag)](https://github.com/AndrewBiz/etng/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

ExifTool нового поколения: быстрый CLI на Rust с выводом, совместимым
с `exiftool` — JSON, CSV, текстом и сырыми байтами. Чтение метаданных и
запись дат. Без Perl, с нативным
разбором контейнеров и опциональным fallback на системный exiftool.

**Статус:** alpha (0.10.0). Ядро чтения стабильно, покрытие форматов расширяется.

GitHub: https://github.com/AndrewBiz/etng · GitVerse: https://gitverse.ru/kazimir_bizik/etng

## Зачем

- **Скорость.** Одиночный JPEG — ~8 мс против ~100 мс у exiftool; 10 000 JPEG —
  ~0.3 с против ~49 с. CR3 нативно — ~1.6 мс.
- **Один статический бинарник.** Ни Perl, ни модулей; таблицы тегов встроены.
- **Машинный вывод.** JSON, CSV, `-b` (base64/сырые байты), детерминированный
  порядок, коды возврата для скриптов.
- **Честный fallback.** Неподдерживаемые форматы и сложные случаи можно
  делегировать системному exiftool (`auto|never|always`).

## Возможности

- **Форматы:** JPEG, TIFF, DNG, CR2, CR3, NEF, ARW (частично), HEIC/HEIF.
- **Метаданные:** EXIF, IPTC, XMP, GPS, ICC-профили, MakerNotes Canon/Nikon/Sony/Apple,
  IFD1-миниатюры, QuickTime/Track для CR3, QuickTime/HEVC для HEIC.
- **Вывод:**
  - `-j` — JSON, совместимый с `exiftool -j`: порядок полей, `-G`/`-G1`, `-s`, `-n`,
    печатные конверсии, `ExifToolVersion` = версия совместимости (13.55);
  - `-csv` — таблица `SourceFile` + колонки; без выбора тегов union по алфавиту,
    с `-TAG` — порядок аргументов; экранирование и base64 для `-b`;
  - `-b` — сырые байты: миниатюры, `PreviewImage` (CR2/CR3/MPF-JPEG), base64 в JSON;
  - текст — `Tag : value` по файлу.
- **Фильтры:** `-TAG` / `-GROUP:TAG`, `-x TAG` / `-x GROUP:TAG`, `-r`, `-ext`.
- **Запись дат:** `DateTimeOriginal`, `CreateDate`, `ModifyDate`, `AllDates`,
  `FileModifyDate`; атомарная замена файла.
- **Пакетная обработка:** параллельная, с детерминированным порядком вывода.

## Установка

### Готовая сборка

Linux x86_64, статический бинарник (musl), без зависимостей:

```bash
curl -LO https://github.com/AndrewBiz/etng/releases/download/v0.10.0/etng-0.10.0-x86_64-unknown-linux-musl.tar.gz
tar xzf etng-0.10.0-x86_64-unknown-linux-musl.tar.gz
install -m755 etng-0.10.0-x86_64-unknown-linux-musl/etng ~/.local/bin/etng
```

Контрольная сумма — в `.sha256` рядом с архивом. Все релизы:
https://github.com/AndrewBiz/etng/releases

### Из исходников

```bash
git clone https://github.com/AndrewBiz/etng.git etng && cd etng
cargo build --release
install -m755 target/release/etng ~/.local/bin/etng
```

Или:

```bash
cargo install --path crates/etng-cli
```

Требования: Rust 1.92+. Системный `exiftool` нужен только для fallback
(делегирование неподдерживаемых форматов и полный JSON там, где нативный
парсер ещё не покрывает всё).

## Использование

```bash
etng -j photo.jpg                      # JSON как exiftool -j
etng -j -G1 -Make -Model ~/Photos      # группы и выбор тегов
etng -j -r -ext jpg -ext heic arch/    # рекурсивный пакетный режим
etng -j --fallback=never weird.pdf     # без делегирования

etng -csv -r -ext jpg ~/Photos > all.csv
etng -csv -Make -Model -x Model photo.jpg

etng -b -ThumbnailImage photo.jpg > thumb.jpg
etng -b -PreviewImage raw.cr2 > preview.jpg
etng -j -b -ThumbnailImage photo.jpg   # base64 в JSON

etng -j -x ThumbnailImage photo.jpg    # исключить тег
etng -ver                              # версия совместимости

# Запись дат (MVP, существующие теги в JPEG/TIFF)
etng -DateTimeOriginal='2024:05:06 07:08:09' photo.jpg
etng -AllDates='2024:05:06 07:08:09' -FileModifyDate='2024:05:06 07:08:09' photo.jpg
```

Коды возврата: `0` — успех, `1` — предупреждения/частичный результат,
`2` — ошибка использования.

## Покрытие форматов

| Формат | Статус |
| --- | --- |
| JPEG | нативно; EXIF/IPTC/XMP, MPF-превью (`MPImage2:PreviewImage`) |
| TIFF / DNG / NEF | нативно |
| CR2 | нативно; EXIF/Canon MakerNotes/GPS, `PreviewImage` |
| CR3 | нативно: ISO-BMFF, CMT1–CMT4, QuickTime/Track, `THMB`/`PRVW`; в `auto` JSON/CSV дополняются exiftool до полного Canon-набора |
| ARW | TIFF/Sony, SubIFD и `PreviewImage` (включая `-b`); SR2SubIFD и часть Sony-таблиц пока не читаются |
| HEIC/HEIF | нативно: EXIF/XMP, `ftyp`/`hvcC`/`colr`/`ispe`/`irot`/`pixi`/`auxC`/`mdat` |
| PNG | нативно: IHDR, eXIf, XMP в iTXt/tEXt, sRGB, tIME |
| MOV | нативно: ftyp, mvhd (даты, длительность), mdat; треки/кодеки — нет |
| MTS/M2TS | нативно: demux TS, H.264 SEI MDPM — дата, GPS |

## Ограничения

- **Запись:** только даты и только JPEG/TIFF; HEIC/CR3/MOV не поддержаны.
  Все три EXIF-даты перезаписываются и вставляются при отсутствии.
- **CR3 / Canon:** ColorData для JPEG/CR2 (версии 32/33/34/48/64/65), CTMD
  Track4, FilterInfo, FocusBracketing и LevelInfo уже читаются; полный
  CanonCustom и сложные Composite ещё не разобраны,
  поэтому в режиме `auto` JSON и CSV для CR3 заменяются выводом exiftool, чтобы
  не терять теги. `--fallback=never` показывает нативный набор, `-b` никогда не
  делегирует.
- **ARW:** SubIFD и `PreviewImage` читаются; SR2SubIFD (зашифрован) и часть
  вложенных Sony-таблиц пока отсутствуют.
- **MTS/M2TS** читаются дата и GPS; `M2TS:Duration`, кодеки и треки — нет.
- **MOV** — только заголовок и даты, без треков и кодеков.
- `CompatibleBrands` в HEIC/CR3 выводится строкой, а не JSON-массивом.
- Подробный список осознанных расхождений — в [`docs/divergences.md`](docs/divergences.md).

## Запись дат

MVP пишет существующие даты и вставляет отсутствующие `DateTimeOriginal` и
`CreateDate` в ExifIFD, а `ModifyDate` — в IFD0 (JPEG/TIFF). `AllDates`
разворачивается в три тега; `FileModifyDate` работает отдельно. Замена атомарная (temp + rename), права файла
сохраняются.

Поддерживаемые цели: `DateTimeOriginal`, `CreateDate`, `ModifyDate`,
`AllDates`, `FileModifyDate`. Формат даты: `YYYY:MM:DD HH:MM:SS` или
`YYYY-MM-DD HH:MM:SS`. `-overwrite_original` принимается для совместимости.

## Замеры

Актуальные цифры и методика — в [`BENCHMARKS.md`](BENCHMARKS.md). Кратко
(release, Arch Linux, exiftool 13.55):

- старт `-ver`: 0.9 мс против 66 мс (71×);
- одиночный JPEG: 8.1 мс против 102 мс (12.6×);
- 10 000 JPEG `-j`: 274 мс против 48.8 с (178×), RSS 12 против 27 МБ;
- 10 000 JPEG `-csv`: 486 мс (~97×);
- CR3 `-j --fallback=never`: 1.6 мс против 197 мс;
- CR3 `-b -PreviewImage`: 7.8 мс против 188 мс.

## Разработка

```bash
cargo test --workspace                  # unit + CLI + streaming + mutations
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cargo xtask generate-tags               # пересобрать таблицы из снимка -listx
cargo xtask snapshots                   # эталонные JSON-снимки exiftool
cargo xtask compare                     # дифф против эталонов
python3 tests/compare_corpus.py          # тот же дифф, отчёт по файлам
python3 tests/bench.py                  # замеры против exiftool
```

Структура:

- `crates/etng-core` — модель тегов, парсеры (JPEG/TIFF/HEIC/CR3), JSON/CSV/raw.
- `crates/etng-cli` — бинарник `etng`: аргументы, обход, fallback.
- `crates/etng-tags` — таблицы тегов из `exiftool -listx` (31 942 тега).
- `xtask` — кодогенерация, снимки, компаратор.

Проект ведётся через OpenSpec: изменения и требования — в `openspec/`
(7 capability, все спеки проходят `--strict`).

## Лицензия

- Код — **MIT** (`LICENSE`).
- Данные ExifTool (`crates/etng-tags/data/listx.xml`, `tags.tsv`) —
  **Artistic-1.0-Perl**; пакет `etng-tags` распространяется как
  `MIT AND Artistic-1.0-Perl`.
- Подробности — в `NOTICE`, `LICENSES/` и [`docs/licensing.md`](docs/licensing.md).
