# phtools-extra-tags Specification

## Purpose

Запись оставшихся phtools-тегов в JPEG.

## Requirements

### Requirement: XMP location extras
Система SHALL записывать `CountryCode` и `WorldRegion` в
`Iptc4xmpExt:LocationShownCountryCode` / `LocationShownWorldRegion`.

#### Scenario: Нет XMP
- **WHEN** задан CountryCode
- **THEN** создаётся XMP с iptcExt-свойством

### Requirement: CodedCharacterSet
Система SHALL записывать `CodedCharacterSet` в IPTC 1:90, преобразуя
`UTF8`/`UTF-8` в `\x1b%G`.

#### Scenario: UTF8
- **WHEN** задан CodedCharacterSet=UTF8
- **THEN** exiftool показывает UTF8, digest обновлён

### Requirement: ImageUniqueID
Система SHALL записывать `ImageUniqueID` в ExifIFD 0xA420, вставляя тег
append-only при отсутствии.

#### Scenario: Нет ImageUniqueID
- **WHEN** задан ImageUniqueID
- **THEN** тег появляется, остальные ExifIFD-теги сохраняются
