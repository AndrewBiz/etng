# iptc-date-format Specification

## Purpose

ExifTool-совместимое представление IPTC-дат.

## ADDED Requirements

### Requirement: Normalized IPTC date
etng SHALL выводить `IPTC:DateCreated` и `IPTC:DigitalCreationDate` в формате
`YYYY:MM:DD`.

#### Scenario: 20020202
- **WHEN** в IIM записано `20020202`
- **THEN** JSON содержит `"2002:02:02"`

### Requirement: Normalized IPTC time
etng SHALL выводить `IPTC:TimeCreated` и `IPTC:DigitalCreationTime` в формате
`HH:MM:SS±HH:MM`.

#### Scenario: 020202+0200
- **WHEN** в IIM записано `020202+0200`
- **THEN** JSON содержит `"02:02:02+02:00"`
