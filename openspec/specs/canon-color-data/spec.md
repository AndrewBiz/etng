# canon-color-data Specification

## Purpose

Баланс белого и цветовые температуры из Canon ColorData (`MakerNotes:0x4001`).

## Requirements

### Requirement: ColorData version and white balance
Система SHALL разбирать Canon ColorData для версий 32, 33, 34, 48, 64 и 65 и
MUST выводить `ColorDataVersion`, `WB_RGGBLevels*` и `ColorTemp*` в группе
`Canon`.

#### Scenario: R50 JPEG
- **WHEN** MakerNotes содержит ColorData version 48 с WB 1792/1024/1024/1697
- **THEN** `ColorDataVersion` = `48 (R7/R10/R50/R6mkII)`,
  `WB_RGGBLevelsAsShot` = `1792 1024 1024 1697`, `ColorTempAsShot` = `5503`

#### Scenario: Неизвестная версия
- **WHEN** первая int16 не входит в известные версии
- **THEN** ColorData-теги не выводятся, остальной разбор продолжается

### Requirement: Table selection by version
Система MUST выбирать таблицу смещений по `ColorDataVersion`, а не по количеству
элементов, и MUST читать quad-значения как четыре последовательных int16s.

#### Scenario: Дневной свет
- **WHEN** version 48 и смещение 0xcd содержит 1753/1024/1024/1762
- **THEN** `WB_RGGBLevelsDaylight` = `1753 1024 1024 1762`,
  `ColorTempDaylight` = `5200`
