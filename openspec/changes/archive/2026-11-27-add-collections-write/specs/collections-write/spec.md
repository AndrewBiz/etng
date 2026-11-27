# collections-write Specification

## Purpose

Запись XMP MWG Collections в JPEG.

## ADDED Requirements

### Requirement: Collections structure
Система SHALL записывать `Collections` как `rdf:Bag` из структур
`CollectionName`/`CollectionURI`.

#### Scenario: Одна коллекция
- **WHEN** задано `-Collections=Name|uri`
- **THEN** exiftool показывает CollectionName=Name, CollectionURI=uri

### Requirement: Multiple collections
Повторный `-Collections=` MUST добавлять структуру в список.

#### Scenario: Две коллекции
- **WHEN** заданы два `-Collections`
- **THEN** в XMP два элемента Bag

### Requirement: Missing XMP
Если XMP нет, система SHALL создать пакет с mwg-coll namespace.

#### Scenario: Нет XMP
- **WHEN** JPEG без XMP
- **THEN** Collections доступны exiftool
