# tag-list Specification

## Purpose

Совместимость с `exiftool -list`/`-listw`.

## Requirements

### Requirement: List tags
etng SHALL поддерживать `-list` и печатать известные имена тегов в формате,
разбираемом ExifTool-совместимыми клиентами.

#### Scenario: MiniExiftool cache
- **WHEN** MiniExiftool выполняет `exiftool -list`
- **THEN** вывод содержит строки с именами тегов, включая `DateTimeOriginal`

### Requirement: List writable tags
etng SHALL поддерживать `-listw` без ошибки.

#### Scenario: Cache build
- **WHEN** MiniExiftool выполняет `exiftool -listw`
- **THEN** команда завершается с кодом 0 и непустым списком
