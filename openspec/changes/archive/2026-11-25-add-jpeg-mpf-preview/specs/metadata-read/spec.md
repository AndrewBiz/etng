# Spec Delta

## ADDED Requirements

### Requirement: JPEG MPF preview images
Система SHALL извлекать вторичные изображения из APP2-сегмента MPF и MUST выводить их как `MPImageN:PreviewImage` с сырыми байтами, доступными через `-b`.

#### Scenario: Canon JPEG с MPF
- **WHEN** JPEG содержит APP2 `MPF\0` с двумя изображениями
- **THEN** `-b -PreviewImage` возвращает JPEG второго изображения, начинаясь с `FFD8`

#### Scenario: Нет MPF
- **WHEN** JPEG не содержит MPF
- **THEN** теги `MPImage*` не добавляются, остальные метаданные не меняются

### Requirement: CR3 thumbnail tail
Система MUST извлекать `Canon:ThumbnailImage` из `THMB` как `payload[16..]`, включая завершающий padding, чтобы длина совпадала с exiftool.

#### Scenario: CR3 THMB с padding
- **WHEN** `THMB` содержит JPEG и нулевой хвост
- **THEN** длина `-b -ThumbnailImage` равна `box_size - 24`
