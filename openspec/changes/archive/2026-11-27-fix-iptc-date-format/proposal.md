# Proposal

## Why

ExifTool нормализует IPTC-даты в `YYYY:MM:DD`, а время в `HH:MM:SS±HH:MM`
(`IPTC.pm`: `ExifDate`/`ExifTime`). etng отдавал сырые `20020202` и
`020202+0200`, из-за чего phtools/MiniExiftool видел строку/число вместо даты
и пропускал IPTC `DateCreated`.

## What Changes

- `IPTC:DateCreated`/`DigitalCreationDate` → `2002:02:02`.
- `IPTC:TimeCreated`/`DigitalCreationTime` → `02:02:02+02:00`.
- Тесты на формат + обновление ожиданий write-тестов.

## Capabilities

### New Capabilities

- `iptc-date-format`: ExifTool-совместимый вывод IPTC-дат.

## Impact

- `crates/etng-core/src/iptc.rs`, CLI-тесты; phtools read.
