# Changelog

## 0.21.1 — 2026-11-27

### Исправлено

- `etng -list` / `-listw` — кэш имён тегов для MiniExiftool; без него
  `phgettags --full_dump` на чистой машине печатал один `nil`-ключ.
- CI phtools проходит на чистом кэше: 12/12.

## 0.21.0 — 2026-11-27

### CI

- Новая job `phtools`: Ruby 3.4, phtools pinned commit `8e3f18c`,
  `phgettags` + `phfixdto` через etng shim.
- Интеграционный baseline: 12/12 сценариев.

## 0.20.2 — 2026-11-27

### Исправлено

- `etng -s -FileModifyDate` печатает выбранное системное поле в text-режиме;
  это чинит проверку `phfixfmd` в интеграции phtools.
- Полный phtools-набор через shim: 64/65 (остался сценарий с IPTC
  `DateCreated`, который падает и с настоящим exiftool 13.55).

## 0.20.1 — 2026-11-27

### Исправлено

- Shim отбрасывает exiftool-only read-опции (`-c` с форматом и др.),
  которые ломали `phgettags`/`phfixdto`.
- `__pycache__` убран из репозитория.

## 0.20.0 — 2026-11-27

### Интеграция с phtools

- `tools/exiftool-shim/exiftool`: read → etng, `-@` script → etng.
- Поддержка MWG/XMP/IPTC/GPS/Collections и
  `-FileModifyDate<DateTimeOriginal`.
- `tools/phtools-integration.sh`: PATH shim + cucumber.
- Strict fallback off по умолчанию; smoke-тесты в CI.
- 123 теста + 2 shim-теста.

## 0.19.0 — 2026-11-27

### MWG Collections

- `-Collections=Name|URI`, можно повторять.
- XMP `mwg-coll:Collections` — `rdf:Bag` из структур
  `CollectionName`/`CollectionURI`.
- XMP создаётся при отсутствии; существующий список заменяется.
- 123 теста.

## 0.18.1 — 2026-11-27

- GPS-тест использует tracked TIFF fixture (CI checkout не содержит
  `Canon_40D.jpg`).

## 0.18.0 — 2026-11-27

### GPS write

- `GPSLatitude`/`GPSLongitude` — rational64u, refs N/S/E/W.
- `GPSAltitude`/`GPSAltitudeRef`.
- GPS IFD append-only; при отсутствии создаётся, указатель `0x8825`
  добавляется в IFD0.
- Поддержка десятичных градусов и DMS.
- 121 тест.

## 0.17.0 — 2026-11-27

### Оставшиеся phtools-теги

- `CountryCode`/`WorldRegion` → XMP `Iptc4xmpExt`.
- `CodedCharacterSet` → IPTC 1:90 (`UTF8` → `\x1b%G`).
- `ImageUniqueID` → ExifIFD `0xA420`, с append-only вставкой.
- 120 тестов.

## 0.16.1 — 2026-11-27

### IPTCDigest

- После изменения IPTC пересчитывается MD5 IIM и обновляется Photoshop
  resource `0x0425`; при отсутствии — добавляется.
- exiftool больше не предупреждает `IPTCDigest is not current` после записи.
- 118 тестов.

## 0.16.0 — 2026-11-26

### Вставка дат HEIC/CR3

- HEIC: отсутствующий EXIF-тег добавляется как новый `mdat` item; `iloc`
  переводится на него.
- CR3: CMT2/CMT1 пересобираются, обновляются `moov` и `stco`/`co64`.
- Для отсутствующего DTO/CreateDate в IFD0 — append-only вставка.
- MOV использует существующие `mvhd`-даты.
- 117 тестов.

## 0.15.0 — 2026-11-26

### HEIC/CR3/MOV write

- HEIC: существующие EXIF-даты обновляются in-place.
- CR3: CMT2 (DTO/CreateDate), CMT1 (ModifyDate), mvhd.
- MOV: mvhd CreateDate/ModifyDate (UTC).
- Вставка отсутствующих дат в BMFF пока не поддержана.
- 117 тестов.

## 0.14.1 — 2026-11-26

- Rust toolchain/MSRV обновлены до 1.100.0 (rust-toolchain.toml, CI).
- Исправлены новые clippy-предупреждения 1.100 (`chunks_exact`/`as_chunks`,
  collapsible if/match).
- Удалены неиспользуемые workspace-зависимости `img-parts`, `rayon`.

## 0.14.0 — 2026-11-26

### Multi-value Keywords/Creator

- Повторные `-Keywords=`/`-Creator=` собираются в список.
- XMP `dc:subject` — `rdf:Bag`, `dc:creator` — `rdf:Seq`.
- IPTC 2:25/2:80 — отдельная запись на значение.
- EXIF Artist — значения через `; `.
- 114 тестов.

## 0.13.0 — 2026-11-26

### MWG-теги

- `City`, `Country`, `State`, `Location`, `Keywords`, `Creator`, `Copyright`.
- XMP: photoshop City/Country/State, Iptc4xmpCore Location, dc subject/creator/
  rights.
- IPTC: 2:90/2:95/2:101/2:92/2:25/2:80/2:116.
- EXIF Artist/Copyright: обновление и append-only вставка.
- Создание XMP/IPTC при отсутствии.
- 113 тестов.

## 0.12.0 — 2026-11-26

### Создание XMP/IPTC

- XMP-пакет создаётся, если отсутствует: DateCreated/CreateDate/ModifyDate.
- APP13/IPTC создаётся, если отсутствует: DateCreated/TimeCreated и
  DigitalCreation*.
- Недостающие свойства/datasets добавляются в существующие пакеты.
- Новые сегменты вставляются после SOI с пересчётом длины.
- 112 тестов.

## 0.11.0 — 2026-11-26

### XMP/IPTC даты

- Обновляются существующие XMP `photoshop:DateCreated`, `xmp:CreateDate`,
  `xmp:ModifyDate`.
- Обновляются IPTC `DateCreated`/`TimeCreated`,
  `DigitalCreationDate`/`DigitalCreationTime`; смещение времени сохраняется.
- EXIF и sidecar-обновления применяются одним atomic replace.
- 109 тестов.

## 0.10.0 — 2026-11-26

### ModifyDate и AllDates

- Отсутствующий `ModifyDate` вставляется в IFD0 (append-only IFD0).
- TIFF header переводится на новый IFD0; JPEG APP1 пересчитывается.
- `AllDates` обновляет/вставляет DateTimeOriginal, CreateDate, ModifyDate.
- 105 тестов.

## 0.9.0 — 2026-11-26

### Вставка дат

- Отсутствующие `DateTimeOriginal`/`CreateDate` вставляются в ExifIFD.
- Append-only IFD: существующие значения и offset'ы не двигаются; указатель
  `0x8769` в IFD0 обновляется.
- JPEG APP1 length пересчитывается, TIFF дополняется.
- Смешанный запрос обновляет существующие даты и вставляет отсутствующие.
- `ModifyDate` при отсутствии пока не вставляется.

### Корпус

- Добавлены 25 фикстур phtools (даты/теги) с MIT provenance.

### Проверки

- 103 теста, clippy/fmt чистые, `xtask compare` — 17 файлов, 0 расхождений.

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
