# Proposal

## Why

HEIC читался только по EXIF/XMP: отсутствовали `ftyp`, `meta` handler, свойства `ipco` (`hvcC`, `colr`, `ispe`, `irot`, `pixi`, `auxC`, `clli`) и `mdat`. На iPhone-файлах это 27–35 тегов QuickTime, включая профиль HEVC, цвет, поворот и размеры.

## What Changes

- Разбор `ftyp`: MajorBrand, MinorVersion, CompatibleBrands.
- `meta/hdlr` → HandlerType; `pitm` → `Meta:PrimaryItemReference`.
- `iprp/ipco/ipma`: свойства primary item — `ispe` (ImageSpatialExtent), `irot` (Rotation), `pixi` (ImagePixelDepth), `auxC`, `colr` nclx (primaries/transfer/matrix/full range), `clli`.
- `hvcC`: profile/tier/IDC, compatibility flags, constraints, level, chroma, bit depth, frame rate, temporal layers.
- `mdat`: MediaDataSize/MediaDataOffset.
- `CompatibleBrands` остаётся строкой (в модели нет JSON-массивов) — документированное расхождение.

## Capabilities

### New Capabilities

- `heic-quicktime`: QuickTime/HEVC-метаданные HEIC.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core`: `heic.rs`, вызов из `read.rs`.
- Тесты: HEIC QuickTime-поля.
- Документация: `docs/divergences.md`.
