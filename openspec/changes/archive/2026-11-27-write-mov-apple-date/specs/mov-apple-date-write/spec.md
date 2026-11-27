# mov-apple-date-write Specification

## Purpose

Запись Apple `Keys:CreationDate` в MOV.

## ADDED Requirements

### Requirement: Update existing Apple creation date
При записи `CreateDate`/`DateTimeOriginal` в MOV etng SHALL обновлять
существующий `com.apple.quicktime.creationdate` in-place.

#### Scenario: iPhone MOV
- **WHEN** файл содержит `2001-01-01T01:01:01+0300` и записана дата
  `2024:05:06 07:08:09`
- **THEN** `Keys:CreationDate` равен `2024:05:06 07:08:09+03:00`

### Requirement: Preserve timezone offset
etng SHALL сохранять offset существующего Apple-значения.

#### Scenario: +0300
- **WHEN** старое значение имеет offset `+0300`
- **THEN** новое значение оканчивается на `+0300` (в выводе `+03:00`)

### Requirement: Missing key is not created
Если Apple-ключа нет, запись MUST NOT добавлять его и MUST NOT ломать `mvhd`.

#### Scenario: MOV без meta
- **WHEN** в файле только `mvhd`
- **THEN** запись `CreateDate` успешна, `QuickTime:CreateDate` обновлён
