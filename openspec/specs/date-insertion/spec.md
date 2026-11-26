# date-insertion Specification

## Purpose

Вставка отсутствующих EXIF-дат в JPEG/TIFF.

## Requirements

### Requirement: Insert missing ExifIFD dates
Система SHALL вставлять отсутствующие `DateTimeOriginal` и `CreateDate` в
ExifIFD, не перемещая существующие значения, и MUST обновлять указатель
`0x8769` в IFD0.

#### Scenario: Нет DateTimeOriginal
- **WHEN** JPEG не содержит DateTimeOriginal и передано значение
- **THEN** тег появляется, остальные теги (Make/Model/MakerNotes) сохраняются

#### Scenario: Нет CreateDate
- **WHEN** JPEG не содержит CreateDate
- **THEN** CreateDate вставляется в ExifIFD

### Requirement: Container growth
Для JPEG система MUST обновить длину APP1; для TIFF — дополнить файл.

#### Scenario: JPEG APP1
- **WHEN** новый ExifIFD добавляется в JPEG
- **THEN** APP1 length корректен, изображение читается exiftool без ошибок

### Requirement: Existing values
При смешанном запросе система MUST обновить существующие даты и вставить
отсутствующие.

#### Scenario: Частичное наличие
- **WHEN** CreateDate есть, DateTimeOriginal нет
- **THEN** оба содержат новое значение
