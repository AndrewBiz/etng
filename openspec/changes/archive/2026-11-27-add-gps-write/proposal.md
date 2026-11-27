# Proposal

## Why

phtools `GpsCreated` пишет GPSLatitude/Longitude/Altitude и refs; etng умел
эти теги только читать.

## What Changes

- Новые writable-теги: `GPSLatitude`, `GPSLatitudeRef`, `GPSLongitude`,
  `GPSLongitudeRef`, `GPSAltitude`, `GPSAltitudeRef`.
- GPS IFD создаётся/пересобирается append-only; значения `rational64u`.
- Поддержка десятичных градусов и DMS (`55.75`, `55 45 0`).
- Ref: N/S/E/W или North/South/East/West; altitude ref 0/1 или текстом.

## Capabilities

### New Capabilities

- `gps-write`: запись GPS в EXIF.

## Impact

- `write.rs`, CLI/help, тест.
