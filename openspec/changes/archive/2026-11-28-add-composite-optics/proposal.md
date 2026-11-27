# Proposal

## Why

В аудите top missing — оптические Composite-теги: `ScaleFactor35efl`,
`CircleOfConfusion`, `FOV`, `HyperfocalDistance`, `GPSAltitude`, `SubSec*`.
Без них etng теряет ~100 тегов на корпусе.

## What Changes

- `Composite:ScaleFactor35efl` из `FocalLengthIn35mmFormat/FocalLength` или
  focal-plane resolution/sensor size.
- `CircleOfConfusion`, `FOV`, `HyperfocalDistance` по формулам ExifTool.
- `Composite:GPSAltitude` с print как у exiftool.
- `SubSecDateTimeOriginal`/`SubSecCreateDate`/`SubSecModifyDate`.
- WB RedBalance/BlueBalance для доступных `WB_RGGBLevels`.

## Capabilities

### New Capabilities

- `composite-optics`: оптические и SubSec Composite-теги.

## Impact

- `crates/etng-core/src/read.rs`, CLI-тест.
