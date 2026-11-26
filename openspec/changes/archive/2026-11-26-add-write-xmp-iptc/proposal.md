# Proposal

## Why

phtools `phfixdto` использует MWG-теги и обновляет EXIF, XMP и IPTC. После
вставки EXIF-дат остаются файлы, у которых дата в XMP/IPTC продолжает
отличаться.

## What Changes

- Обновление существующих XMP-полей JPEG: `photoshop:DateCreated`,
  `xmp:CreateDate`, `xmp:ModifyDate`.
- Обновление существующих IPTC IIM полей: `DateCreated`/`TimeCreated`,
  `DigitalCreationDate`/`DigitalCreationTime`.
- Даты в XMP пишутся в ISO (`YYYY-MM-DDTHH:MM:SS`), в IPTC — `YYYYMMDD` и
  `HHMMSS±HHMM`; смещение существующего IPTC-времени сохраняется.
- Новые XMP/IPTC-пакеты не создаются.

## Capabilities

### New Capabilities

- `date-sidecars`: обновление XMP/IPTC дат.

## Impact

- `crates/etng-core/src/date_sidecars.rs`, `write.rs`.
