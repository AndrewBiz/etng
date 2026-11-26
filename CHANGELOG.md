# Changelog

## 0.2.0 — 2026-11-26

### Canon

- ColorData (`MakerNotes:0x4001`) для версий 32/33/34/48/64/65:
  `ColorDataVersion`, `WB_RGGBLevels*`, `ColorTemp*`.
- Значения совпадают с exiftool на JPEG R50 (version 48).
- CR3 Track4 ColorData и FilterInfo пока не покрыты; `auto` дополняет exiftool.

### Проверки

- 88 тестов, clippy/fmt чистые, `xtask compare` — 16 файлов, 0 расхождений.

## 0.1.0 — 2026-11-26

Первый публичный релиз.

### Возможности

- Read-only CLI `etng` с выводом, совместимым с `exiftool`: JSON (`-j`),
  CSV (`-csv`), текст, сырые байты (`-b`).
- Группы `-G`/`-G1`, `-s`, `-n`, выбор `-TAG`/`-GROUP:TAG`, исключение
  `-x TAG`/`-x GROUP:TAG`, рекурсия `-r`, фильтр `-ext`.
- Нативный разбор JPEG, TIFF, DNG, CR2, CR3, NEF, HEIC/HEIF; частично ARW.
- EXIF, IPTC, XMP, GPS, MakerNotes Canon/Nikon/Sony/Apple, IFD1-миниатюры.
- CR3: ISO-BMFF, `CMT1`–`CMT4`, QuickTime-треки, `THMB`/`PRVW`.
- HEIC: QuickTime/HEVC (`ftyp`, `hvcC`, `colr`, `ispe`, `irot`, `pixi`,
  `auxC`, `clli`, `mdat`).
- MPF-превью JPEG (`MPImage2:PreviewImage`), `PreviewImage` CR2/CR3.
- Fallback на системный exiftool: `auto` (по умолчанию), `never`, `always`.
- Параллельная пакетная обработка с детерминированным порядком.

### Производительность

- Старт: ~0.9 мс против ~66 мс у exiftool.
- Одиночный JPEG: ~8 мс против ~102 мс.
- 10 000 JPEG `-j`: ~274 мс против ~49 с.
- CR3 нативно: ~1.6 мс; `-b -PreviewImage`: ~7.8 мс.

### Известные ограничения

- CR3 в режиме `auto` дополняется выводом exiftool: Canon ColorData/FilterInfo
  ещё не разобраны.
- ARW: SubIFD и часть вложенных Sony-таблиц не читаются.
- PNG, MOV, MTS — только через fallback.
- `CompatibleBrands` выводится строкой, `Meta:MetaImageSize` не декодируется.

### Проверки

- 86 тестов, `clippy -D warnings`, `rustfmt --check`.
- `cargo xtask compare`: 16 файлов корпуса, 0 расхождений.

### Лицензии

- Код — MIT.
- Данные ExifTool — Artistic-1.0-Perl; `etng-tags` — `MIT AND Artistic-1.0-Perl`.
