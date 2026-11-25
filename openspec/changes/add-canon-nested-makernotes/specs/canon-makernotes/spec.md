# canon-makernotes Specification

## Purpose

Разбор вложенных бинарных таблиц Canon MakerNotes и их value/print-конверсий без делегирования exiftool.

## ADDED Requirements

### Requirement: Canon binary sub-tables
Система SHALL разбирать вложенные Canon-таблицы `CameraSettings`, `ShotInfo`, `Processing`, `SensorInfo`, `FocalLength`, `AFInfo`, `AFInfo2`, `AFConfig`, `AFMicroAdj`, `LightingOpt`, `VignettingCorr2`, `TimeInfo`, `MultiExp`, `HDRInfo`, `Ambience` и MUST выдавать теги с группой `MakerNotes`/`Canon` (или `CanonCustom` для функций).

#### Scenario: AFInfo2
- **WHEN** CMT3 содержит AFInfo2 с `NumAFPoints > 0`
- **THEN** вывод содержит `Canon:AFAreaMode`, `Canon:NumAFPoints`, `Canon:AFAreaWidths`, `Canon:AFAreaHeights`, `Canon:AFAreaXPositions`, `Canon:AFAreaYPositions`

#### Scenario: LightingOpt и TimeInfo
- **WHEN** CMT3 содержит соответствующие контейнеры
- **THEN** вывод содержит `Canon:PeripheralIlluminationCorr`, `Canon:AutoLightingOptimizer`, `Canon:TimeZone` в формате `±HH:MM`, `Canon:TimeZoneCity`

### Requirement: Canon value conversions
Система SHALL применять Canon-конверсии: APEX для `FNumber`/`MaxAperture`/`MinAperture`, `ExposureTime` как `1/2^(raw/32)`, `ExposureCompensation` со знаком, `MeasuredEV`, температуру, `SelfTimer`, `FocalUnits`, `ImageUniqueID` в hex и обрезку строк по NUL.

#### Scenario: APEX-апертура
- **WHEN** `FNumber` хранится как 96
- **THEN** в выводе `2.8`

#### Scenario: Уникальный ID
- **WHEN** `ImageUniqueID` содержит 16 байт
- **THEN** значение выводится как 32 hex-символа без пробелов

### Requirement: Suppress parser artefacts
Система MUST NOT выводить служебные/неизвестные теги, которые exiftool для данной модели не эмитит (`AFInfoSize`, `FileNumber`, `ShutterCount`, дубли `FocalPlaneY*` и т.п.).

#### Scenario: AFInfoSize
- **WHEN** разобран AFInfo2
- **THEN** `Canon:AFInfoSize` отсутствует в выводе
