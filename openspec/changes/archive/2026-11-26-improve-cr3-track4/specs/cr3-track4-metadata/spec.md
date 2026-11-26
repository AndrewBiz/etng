# cr3-track4-metadata Specification

## Purpose

Метаданные CTMD-трека CR3, creative-фильтры, focus bracketing и level info.

## ADDED Requirements

### Requirement: Track4 CTMD sample
Система SHALL находить CTMD-сэмпл четвёртого трека, извлекать вложенные
Canon-блоки и MUST выводить их теги в группе `Track4` без дублей.

#### Scenario: R50 ColorData
- **WHEN** CR3 содержит Track4 CTMD с ColorData11
- **THEN** вывод содержит `Track4:ColorDataVersion`,
  `Track4:WB_RGGBLevelsAsShot` и `Track4:ColorTempAsShot`

### Requirement: Filter and focus metadata
Система SHALL разбирать FilterInfo (`0x4024`), FocusBracketingInfo (`0x4053`) и
LevelInfo (`0x4059`) и MUST применять печатные конверсии Off/On, Horizontal/
Vertical, Roll/Pitch.

#### Scenario: Фильтры выключены
- **WHEN** фильтр Grainy B/W имеет значение -1
- **THEN** `Canon:GrainyBWFilter` = `Off`

#### Scenario: Наклон камеры
- **WHEN** LevelInfo RollAngle = -3, PitchAngle = 317
- **THEN** `Canon:RollAngle` = 0.3, `Canon:PitchAngle` = 31.7

### Requirement: Track4 timing and composites
Система SHALL выводить `Track4:SampleTime`/`SampleDuration` и для CR3 MUST
добавлять Composite `ISO`, `Lens`, `Rotation`, `WB_RGGBLevels`, `RedBalance`,
`BlueBalance`, если данные доступны.

#### Scenario: Баланс белого
- **WHEN** WB_RGGBLevelsAsShot = `1811 1024 1024 1680`
- **THEN** `Composite:WB_RGGBLevels` совпадает со строкой,
  `RedBalance` ≈ 1.768555, `BlueBalance` ≈ 1.640625
