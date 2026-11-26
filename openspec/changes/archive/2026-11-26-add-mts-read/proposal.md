# Proposal

## Why

MTS/M2TS уходил в fallback: даты и GPS из AVCHD-видео не читались нативно.
На корпусе это `H264:DateTimeOriginal` и 5 GPS-тегов у одного MTS.

## What Changes

- Детект M2TS/MPEG-TS по 192/188-байтным пакетам и расширению.
- Демультиплексирование: снятие 4-байтного M2TS timestamp, adaptation field,
  PES-заголовков; сбор ES по PID.
- Снятие H.264 emulation-prevention (`00 00 03` → `00 00`).
- Поиск MDPM (`UUID + MDPM`) и разбор записей:
  - `H264:DateTimeOriginal` (теги 0x18+0x19, BCD и timezone/DST);
  - `GPS:GPSVersionID`, `GPSLatitude/Ref`, `GPSLongitude/Ref`.
- `M2TS:Duration`/кодеки — отдельно, если понадобятся.

## Capabilities

### New Capabilities

- `mts-read`: MTS/M2TS + H.264 SEI MDPM.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/mts.rs`, `sniff.rs`, `read.rs`.
- На реальном MTS дата и GPS совпадают с exiftool.
