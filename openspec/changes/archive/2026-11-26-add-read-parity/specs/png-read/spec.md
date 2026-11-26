# png-read Specification

## Purpose

Нативное чтение метаданных PNG.

## ADDED Requirements

### Requirement: PNG chunks
Система SHALL читать IHDR, eXIf, sRGB и tIME и MUST выводить
`PNG:ImageWidth`, `ImageHeight`, `BitDepth`, `ColorType`, `Compression`,
`Filter`, `Interlace` и `SRGBRendering`.

#### Scenario: IHDR
- **WHEN** PNG 1242×2208, 8 бит, RGB
- **THEN** `PNG:ImageWidth` = 1242, `PNG:ColorType` = `RGB`,
  `PNG:BitDepth` = 8

### Requirement: Embedded EXIF and XMP
Система MUST передавать eXIf в общий EXIF-парсер и читать XMP из
iTXt/tEXt с ключом `XML:com.adobe.xmp`.

#### Scenario: Screenshot
- **WHEN** PNG содержит eXIf с `DateTimeOriginal`
- **THEN** `ExifIFD:DateTimeOriginal` присутствует, а XMP-теги попадают в
  группы `XMP-*`
