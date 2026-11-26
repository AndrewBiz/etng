# sidecar-creation Specification

## Purpose

Создание XMP/IPTC дат в JPEG при отсутствии пакетов.

## ADDED Requirements

### Requirement: Create XMP packet
Если XMP-пакет отсутствует, система SHALL создать минимальный пакет с
запрошенными датами.

#### Scenario: Нет XMP
- **WHEN** JPEG без XMP и задан AllDates
- **THEN** XMP содержит DateCreated, CreateDate, ModifyDate

### Requirement: Create IPTC resource
Если APP13/IPTC отсутствует, система SHALL создать Photoshop resource 0x0404
с датами.

#### Scenario: Нет IPTC
- **WHEN** JPEG без IPTC и задан AllDates
- **THEN** IPTC содержит DateCreated/TimeCreated и DigitalCreation*

### Requirement: Extend existing packets
Система MUST добавлять отсутствующие поля в существующие XMP/IPTC, сохраняя
остальные значения.

#### Scenario: Частичный IPTC
- **WHEN** есть только DigitalCreationDate, задан DateTimeOriginal
- **THEN** добавляются DateCreated/TimeCreated, digital-поля сохраняются
