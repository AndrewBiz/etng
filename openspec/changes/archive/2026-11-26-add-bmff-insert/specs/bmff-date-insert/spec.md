# bmff-date-insert Specification

## Purpose

Вставка отсутствующих EXIF-дат в HEIC/CR3.

## ADDED Requirements

### Requirement: HEIC insertion
Если EXIF-тег отсутствует, система SHALL добавить его, не перемещая
существующие item'ы.

#### Scenario: Нет DateTimeOriginal
- **WHEN** HEIC без DateTimeOriginal
- **THEN** тег появляется, exiftool Validate не показывает структурных ошибок

### Requirement: CR3 insertion
Если CMT2/CMT1 не содержит дату, система SHALL вставить её и обновить размеры
box/moov и chunk-offset'ы.

#### Scenario: Нет DateTimeOriginal в CR3
- **WHEN** CMT2 без DateTimeOriginal
- **THEN** тег появляется, QuickTime-даты и изображение читаются

### Requirement: No MOV insertion
MOV SHALL обновлять существующие `mvhd`-даты; отдельная вставка не требуется.

#### Scenario: MOV
- **WHEN** задан CreateDate
- **THEN** QuickTime CreateDate обновлён
