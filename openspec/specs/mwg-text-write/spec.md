# mwg-text-write Specification

## Purpose

Запись MWG-текстовых тегов в JPEG/TIFF.

## Requirements

### Requirement: MWG text assignments
Система SHALL принимать `-City=`, `-Country=`, `-State=`, `-Location=`,
`-Keywords=`, `-Creator=`, `-Copyright=` и записывать их в соответствующие
XMP/IPTC/EXIF-поля.

#### Scenario: Полный набор
- **WHEN** заданы все семь тегов
- **THEN** XMP, IPTC и EXIF Artist/Copyright содержат значения

### Requirement: EXIF Artist/Copyright
Система MUST обновлять существующие EXIF Artist/Copyright и вставлять
отсутствующие append-only способом.

#### Scenario: Нет Artist
- **WHEN** IFD0 не содержит Artist, задан Creator
- **THEN** Artist появляется, остальные теги и IFD1 сохраняются

### Requirement: Sidecar creation
Если XMP/IPTC отсутствуют, система SHALL создать минимальные пакеты с
запрошенными тегами.

#### Scenario: Нет XMP/IPTC
- **WHEN** JPEG без sidecar-пакетов
- **THEN** после записи XMP и IPTC доступны exiftool
