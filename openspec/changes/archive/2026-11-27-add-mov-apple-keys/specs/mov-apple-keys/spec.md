# mov-apple-keys Specification

## Purpose

Чтение Apple QuickTime `Keys` metadata из MOV.

## ADDED Requirements

### Requirement: Read Apple creation date
etng SHALL читать `com.apple.quicktime.creationdate` и выводить его как
`Keys:CreationDate` в формате ExifTool.

#### Scenario: iPhone MOV
- **WHEN** в `moov/meta/ilst` есть `2021-02-22T17:28:32+0300`
- **THEN** JSON содержит `"Keys:CreationDate": "2021:02:22 17:28:32+03:00"`

### Requirement: Read Apple make/model/software
etng SHALL читать Apple `make`, `model`, `software` как `Keys:Make`,
`Keys:Model`, `Keys:Software`.

#### Scenario: iPhone 6s Plus
- **WHEN** ключи содержат `Apple` и `iPhone 6s Plus`
- **THEN** вывод содержит соответствующие `Keys:*` теги

### Requirement: Keep mvhd dates
Наличие `Keys:CreationDate` MUST NOT убирать `QuickTime:CreateDate`/
`ModifyDate`.

#### Scenario: Оба источника
- **WHEN** файл содержит и `mvhd`, и Apple keys
- **THEN** вывод содержит оба тега
