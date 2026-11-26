# date-sidecars Specification

## Purpose

Обновление XMP и IPTC дат в JPEG.

## ADDED Requirements

### Requirement: Update XMP dates
Система SHALL обновлять существующие XMP-свойства `photoshop:DateCreated`,
`xmp:CreateDate`, `xmp:ModifyDate` при записи соответствующих дат.

#### Scenario: XMP DateCreated
- **WHEN** JPEG содержит `photoshop:DateCreated` и задан DateTimeOriginal
- **THEN** XMP-значение становится ISO-датой нового времени

### Requirement: Update IPTC dates
Система SHALL обновлять существующие IPTC datasets `2:55`, `2:60`, `2:62`,
`2:63` для DateTimeOriginal/CreateDate.

#### Scenario: IPTC DateCreated
- **WHEN** JPEG содержит IPTC DateCreated/TimeCreated
- **THEN** дата и время обновлены, смещение времени сохранено

### Requirement: Atomic write
EXIF и sidecar-обновления MUST применяться одним atomic replace.

#### Scenario: Ошибка XMP
- **WHEN** XMP-пакет невалиден
- **THEN** файл не изменяется
