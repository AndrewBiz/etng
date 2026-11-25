# Proposal

## Why

Native CR3 reading covers CMT1–CMT4 and QuickTime, but nested Canon binary tables (AFInfo/AFConfig/LightingOpt/TimeInfo/…) were only partially parsed. Because of that, `auto` mode still replaced CR3 JSON with the exiftool delegate to preserve ~90 tags.

## What Changes

- Native parsing for Canon sub-tables: `AFInfo`, `AFInfo2`, `FocalLength`, `AFMicroAdj`, `AFConfig`, `LightingOpt`, `VignettingCorr2`, `TimeInfo`, `MultiExp`, `HDRInfo`, `Ambience`, `CameraSettings`, `ShotInfo`, `Processing`, `SensorInfo`, file/crop/aspect tables.
- Canon value conversions and prints: exposure/aperture APEX, self-timer, focal units, EV, temperature, model IDs, lens string trimming, image unique id hex.
- Suppression of parser artefacts that exiftool does not emit (unknown AF sizes, duplicate file info).
- CR3 native output reaches Canon 185 tags (exiftool: 183) and 396 fields total.
- Remaining gaps: CanonCustom function values, several Composite derivations, `JpgFromRaw`/warnings. Fallback enrichment stays until they are closed.

## Capabilities

### New Capabilities

- `canon-makernotes`: правила разбора вложенных Canon-таблиц и их value/print-конверсий.

### Modified Capabilities

- `cr3-read`: вложенные Canon-таблицы читаются нативно.

## Impact

- `crates/etng-core`: `cr3.rs`, мелкие правки `read.rs` (приоритет ExifIFD для композитов).
- Тесты: native CR3 без fallback, значения AF/Time/Lighting.
- Документация: `docs/divergences.md`.
