# modifydate-insertion Specification

## Purpose

Вставка отсутствующего `ModifyDate` в IFD0 JPEG/TIFF.

## Requirements

### Requirement: Insert missing ModifyDate
Система SHALL вставлять `ModifyDate` в IFD0, если тега нет, и MUST обновлять
первую IFD в TIFF header.

#### Scenario: Нет ModifyDate
- **WHEN** JPEG не содержит IFD0 ModifyDate и передано значение
- **THEN** тег появляется, остальные теги и IFD1 сохраняются

#### Scenario: Существующий ModifyDate
- **WHEN** ModifyDate уже есть
- **THEN** значение перезаписывается без создания нового IFD

### Requirement: AllDates
`AllDates` MUST обновить или вставить DateTimeOriginal, CreateDate и
ModifyDate.

#### Scenario: Все даты отсутствуют
- **WHEN** передано `-AllDates=...`
- **THEN** все три тега содержат новое значение
