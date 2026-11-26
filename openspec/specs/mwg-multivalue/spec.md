# mwg-multivalue Specification

## Purpose

Запись списков Keywords/Creator.

## Requirements

### Requirement: Repeated flags
Система SHALL собирать повторные `-Keywords=`/`-Creator=` в список в порядке
аргументов.

#### Scenario: Два ключевых слова
- **WHEN** заданы `-Keywords=alpha -Keywords=beta`
- **THEN** XMP Subject и IPTC Keywords содержат оба значения

### Requirement: XMP containers
Система MUST писать Keywords как `rdf:Bag`, Creator как `rdf:Seq`.

#### Scenario: Creator
- **WHEN** заданы два Creator
- **THEN** XMP dc:creator содержит два `rdf:li`

### Requirement: EXIF Artist
Система MUST записывать несколько Creator в Artist через `; `.

#### Scenario: Artist
- **WHEN** Creator = Alice, Bob
- **THEN** IFD0 Artist = `Alice; Bob`
