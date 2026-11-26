# Changelog

## 0.8.1 — 2026-11-26

- Исправлена справка CLI: заявлена запись дат, а не read-only.

## 0.8.0 — 2026-11-26

### Запись дат (MVP)

- `-DateTimeOriginal=`, `-CreateDate=`, `-ModifyDate=`, `-AllDates=`,
  `-FileModifyDate=`.
- Перезапись существующих EXIF-дат в JPEG/TIFF-family.
- Атомарная замена файла, сохранение прав; ошибка при отсутствии тега без
  изменения файла.
- `FileModifyDate` через mtime, локальная зона.
- `-overwrite_original` принимается для совместимости.

### Проверки

- 102 теста, clippy/fmt чистые, `xtask compare` — 17 файлов, 0 расхождений.

## 0.7.0 — 2026-11-26

### MTS/M2TS

- Детект 192-байтных M2TS и 188-байтных TS.
- Demux: timestamp, adaptation field, PES, ES по PID.
- H.264 emulation prevention и MDPM.
- `H264:DateTimeOriginal`, `GPS:GPSVersionID`, `GPSLatitude/Ref`,
  `GPSLongitude/Ref`; на реальном MTS совпало с exiftool.
- `M2TS:Duration`, кодеки и треки пока не читаются.

### Проверки

- 96 тестов, clippy/fmt чистые, `xtask compare` — 17 файлов, 0 расхождений.

## 0.6.0 — 2026-11-26

### Чтение

- XMP: вложенные `LocationShown*` и `Collections`, нормализация
  `Iptc4xmpCore`/`Iptc4xmpExt` — 9 тегов phtools закрыты.
- PNG: IHDR, eXIf, XMP из iTXt/tEXt, sRGB, tIME.
- MOV: ftyp, mvhd (CreateDate/ModifyDate/Duration), mdat.
- MTS/M2TS остаётся через fallback.

### Проверки

- 95 тестов, clippy/fmt чистые, `xtask compare` — 17 файлов, 0 расхождений.

## 0.5.0 — 2026-11-26

### CR3

- Track4 CTMD: вложенные Canon-блоки, ColorData и сопутствующие теги в `Track4`.
- `SampleTime`/`SampleDuration`, black/white levels ColorData.
- FilterInfo (10 creative-фильтров), FocusBracketingInfo, LevelInfo
  (RollAngle/PitchAngle/FocalLength2).
- Composite для CR3: ISO, Lens, Rotation, WB_RGGBLevels, Red/BlueBalance.
- На R50: 548 из 577 тегов exiftool (было 393).

### Проверки

- 92 теста, clippy/fmt чистые, `xtask compare` — 16 файлов, 0 расхождений.

## 0.4.0 — 2026-11-26

### ICC

- Профили из JPEG APP2, TIFF `0x8773`, HEIC `colr/prof`.
- 16 тегов заголовка, ProfileDescription/Copyright, white/black point,
  RGB-матрицы, ChromaticAdaptation, TRC, Technology, view/meas.
- Строки `desc`/`text`/`mluc`/`utf8`, XYZ и enum-подписи.
- Точное совпадение с exiftool на Canon 40D, Nikon D70, exif.jpg, exif.heic.
- ICC больше не входит в known-vendor маску `xtask compare`.

### Проверки

- 91 тест, clippy/fmt чистые, `xtask compare` — 16 файлов, 0 расхождений.

## 0.3.0 — 2026-11-26

### Sony ARW

- SubIFD (первый): размеры RAW, CFA, коррекции, strip-параметры — 24 тега.
- `IFD0:PreviewImageStart/Length/PreviewImage`; `-b -PreviewImage` совпадает
  с exiftool побайтово.
- SR2SubIFD (зашифрован) и часть Sony binary tables пока не читаются.

### Проверки

- 90 тестов, clippy/fmt чистые, `xtask compare` — 16 файлов, 0 расхождений.

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
