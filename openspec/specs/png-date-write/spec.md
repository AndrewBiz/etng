# png-date-write Specification

## Purpose

Запись дат в PNG через eXIf.

## Requirements

### Requirement: Write dates to PNG
etng SHALL записывать DateTimeOriginal/CreateDate в ExifIFD и ModifyDate в
IFD0 PNG, обновляя или создавая `eXIf`.

#### Scenario: Existing eXIf
- **WHEN** PNG содержит eXIf
- **THEN** даты обновляются, PNG chunks остаются валидными

#### Scenario: No eXIf
- **WHEN** PNG не содержит eXIf
- **THEN** создаётся eXIf перед IDAT, даты читаются через EXIF

### Requirement: Preserve PNG image
Запись MUST NOT менять IHDR/IDAT/IEND payload.

#### Scenario: Image data
- **WHEN** даты записаны
- **THEN** ширина/высота и image data не меняются
