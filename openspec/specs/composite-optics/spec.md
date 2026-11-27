# composite-optics Specification

## Purpose

Оптические, GPS и SubSec Composite-теги, совместимые с exiftool.

## Requirements

### Requirement: Optical composites
etng SHALL вычислять `ScaleFactor35efl`, `CircleOfConfusion`, `FOV` и
`HyperfocalDistance`, если есть фокусное расстояние и масштаб 35 мм.

#### Scenario: Sony JPEG
- **WHEN** FocalLength=70, FocalLength35=105
- **THEN** scale=1.5, CoC≈0.020 mm, FOV≈19.5 deg

### Requirement: GPS altitude
etng SHALL выводить `Composite:GPSAltitude` с print `<value> m <ref>`.

#### Scenario: Truncated altitude
- **WHEN** GPSAltitude=12.178 m Above Sea Level
- **THEN** print `12.1 m Above Sea Level`

### Requirement: SubSec dates
etng SHALL склеивать базовую дату и `SubSecTime*` в `SubSec*Date` теги.

#### Scenario: CR2
- **WHEN** DateTimeOriginal=2019:04:24 15:20:13, SubSecTimeOriginal=00
- **THEN** `Composite:SubSecDateTimeOriginal` = `2019:04:24 15:20:13.00`
