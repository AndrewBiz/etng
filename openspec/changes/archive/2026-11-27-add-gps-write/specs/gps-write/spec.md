# gps-write Specification

## Purpose

Запись GPS-координат и высоты в EXIF.

## ADDED Requirements

### Requirement: GPS coordinates
Система SHALL записывать `GPSLatitude`/`GPSLongitude` как `rational64u[3]` и
refs как ASCII.

#### Scenario: Десятичные градусы
- **WHEN** задано `GPSLatitude=55.75`
- **THEN** exiftool показывает `55 deg 45' 0.00"`

### Requirement: GPS altitude
Система SHALL записывать `GPSAltitude` (`rational64u`) и
`GPSAltitudeRef` (0/1).

#### Scenario: Высота
- **WHEN** задано `GPSAltitude=150.5`
- **THEN** exiftool показывает 150.5 m Above Sea Level

### Requirement: Missing GPS IFD
Система SHALL создать GPS IFD и указатель `0x8825`, если их не было.

#### Scenario: Нет GPS
- **WHEN** JPEG без GPS
- **THEN** после записи GPS-теги читаются, структура валидна
