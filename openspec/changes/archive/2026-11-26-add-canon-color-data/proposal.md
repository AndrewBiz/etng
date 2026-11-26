# Proposal

## Why

Canon MakerNotes tag `0x4001` (ColorData) содержит баланс белого и цветовые
температуры. На реальных JPEG/CR3 (R50, версия 48) exiftool выводит 12 тегов
`ColorDataVersion`, `WB_RGGBLevels*`, `ColorTemp*`, а etng — ни одного.

## What Changes

- Разбор `0x4001` для версий 32/33 (ColorData10), 34/48 (ColorData11),
  64/65 (ColorData12).
- `ColorDataVersion` с печатной меткой модели.
- `WB_RGGBLevels{AsShot,Auto,Measured,Daylight,Shade,Cloudy,Tungsten,Fluorescent,Kelvin,Flash}`
  как четыре int16s.
- `ColorTemp*` как int16s.
- Значения сверяются с exiftool на JPEG R50; для CR3 Track4 остаётся fallback.

## Capabilities

### New Capabilities

- `canon-color-data`: баланс белого и температуры из Canon ColorData.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/cr3.rs`: ветка `0x4001`, таблицы смещений.
- Тесты: синтетический ColorData11, неизвестная версия.
- Документация: `docs/divergences.md`.
