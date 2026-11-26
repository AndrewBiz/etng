# bmff-date-write Specification

## Purpose

Запись существующих дат в HEIC/CR3/MOV.

## ADDED Requirements

### Requirement: HEIC EXIF dates
Система SHALL обновлять существующие EXIF-даты HEIC in-place.

#### Scenario: DateTimeOriginal
- **WHEN** HEIC содержит DateTimeOriginal
- **THEN** значение обновлено, размер файла не меняется

### Requirement: CR3 EXIF and mvhd
Система SHALL обновлять существующие ExifIFD/IFD0 даты CR3 и `mvhd`.

#### Scenario: AllDates
- **WHEN** задан AllDates
- **THEN** DTO/CreateDate в CMT2, ModifyDate в CMT1, QuickTime-даты в mvhd

### Requirement: MOV header dates
Система SHALL обновлять `mvhd` CreateDate/ModifyDate в MOV.

#### Scenario: MOV
- **WHEN** задан CreateDate
- **THEN** QuickTime CreateDate читается как заданное UTC-время
