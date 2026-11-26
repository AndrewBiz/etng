# Proposal

## Why

После синхронизации существующих XMP/IPTC оставались файлы без этих пакетов
(`7filemodifydate.JPG`, `6iptc_digitalcreationdate.JPG`): дата попадала только
в EXIF, а phtools `phfixdto` пишет MWG-набор во все группы.

## What Changes

- Создание минимального XMP-пакета с `photoshop:DateCreated`,
  `xmp:CreateDate`, `xmp:ModifyDate`.
- Создание APP13/IPTC с `DateCreated`/`TimeCreated`,
  `DigitalCreationDate`/`DigitalCreationTime`.
- Добавление отсутствующих datasets в существующий IPTC и свойств в
  существующий XMP.
- Вставка новых сегментов после SOI, пересчёт длин.

## Capabilities

### New Capabilities

- `sidecar-creation`: создание XMP/IPTC дат при отсутствии.

## Impact

- `crates/etng-core/src/date_sidecars.rs`, CLI-тесты.
