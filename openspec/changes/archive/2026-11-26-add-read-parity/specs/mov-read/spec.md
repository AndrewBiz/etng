# mov-read Specification

## Purpose

Базовые метаданные QuickTime/MOV для задач переименования по дате.

## ADDED Requirements

### Requirement: Movie header
Система SHALL определять MOV по `ftyp` и читать `mvhd` версий 0 и 1, выводя
`QuickTime:CreateDate`, `ModifyDate` и `Duration`.

#### Scenario: MOV с mvhd v0
- **WHEN** MOV содержит `mvhd` с creation time 2017-01-06 02:04:06
- **THEN** `QuickTime:CreateDate` = `2017:01:06 02:04:06`

### Requirement: Media data
Система MUST выводить `MediaDataSize` и `MediaDataOffset` для `mdat`.

#### Scenario: mdat
- **WHEN** MOV содержит `mdat`
- **THEN** `QuickTime:MediaDataSize` равен длине payload
