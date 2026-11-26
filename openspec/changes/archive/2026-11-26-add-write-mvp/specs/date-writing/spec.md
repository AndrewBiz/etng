# date-writing Specification

## Purpose

Запись дат в EXIF и установка FileModifyDate.

## ADDED Requirements

### Requirement: Date assignments
Система SHALL принимать `-DateTimeOriginal=`, `-CreateDate=`, `-ModifyDate=`,
`-AllDates=` и `-FileModifyDate=` и MUST валидировать формат даты.

#### Scenario: Запись DateTimeOriginal
- **WHEN** JPEG содержит DateTimeOriginal и передано
  `-DateTimeOriginal=2024:05:06 07:08:09`
- **THEN** тег читается обратно с новым значением

#### Scenario: AllDates
- **WHEN** передан `-AllDates=...`
- **THEN** обновляются DateTimeOriginal, CreateDate и ModifyDate

### Requirement: No insertion
Система MUST завершаться ошибкой, не изменяя файл, если запрошенного тега нет.

#### Scenario: Нет тега
- **WHEN** в TIFF нет DateTimeOriginal
- **THEN** exit code 1, файл побайтово не изменён

### Requirement: Atomic replacement
Система MUST заменять файл атомарно и сохранять права; при ошибке временный
файл удаляется.

#### Scenario: Успешная запись
- **WHEN** запись завершена
- **THEN** на месте файла новый inode с теми же правами

### Requirement: FileModifyDate
Система SHALL устанавливать mtime из `FileModifyDate` после замены файла.

#### Scenario: mtime
- **WHEN** передан `-FileModifyDate=2024:05:06 07:08:09`
- **THEN** `System:FileModifyDate` начинается с этой даты
