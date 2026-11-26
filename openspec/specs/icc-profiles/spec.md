# icc-profiles Specification

## Purpose

Чтение ICC-профилей из JPEG, TIFF и HEIC и вывод тегов в группах
`ICC-header`, `ICC_Profile`, `ICC-view`, `ICC-meas`.

## Requirements

### Requirement: Profile extraction
Система SHALL извлекать ICC-профиль из JPEG APP2 (`ICC_PROFILE`), TIFF-тега
`0x8773` и HEIC-бокса `colr` с типом `prof`, собирая JPEG-чанки по sequence.

#### Scenario: JPEG APP2
- **WHEN** JPEG содержит два чанка ICC_PROFILE с sequence 1 и 2
- **THEN** профиль собирается целиком и выводится

#### Scenario: HEIC colr
- **WHEN** HEIC `colr` содержит `prof` с профилем Display P3
- **THEN** `ICC_Profile:ProfileDescription` = `Display P3`

### Requirement: Header and common tags
Система SHALL выводить 16 тегов заголовка и MUST разбирать `desc`/`text`/`mluc`
строки, XYZ-значения, TRC как байты и enum-подписи.

#### Scenario: sRGB профиль
- **WHEN** профиль версии 2.1.0, класс `mntr`, intent 0
- **THEN** ProfileVersion = `2.1.0`, ProfileClass = `Display Device Profile`,
  RenderingIntent = `Perceptual`

#### Scenario: TRC
- **WHEN** профиль содержит `rTRC`
- **THEN** `ICC_Profile:RedTRC` — binary placeholder, `-b -RedTRC` отдаёт байты

### Requirement: Viewing and measurement conditions
Система SHALL выводить `view` и `meas` теги, если они есть в профиле.

#### Scenario: sRGB viewing conditions
- **WHEN** профиль содержит `view` с illuminant D50
- **THEN** `ICC-view:ViewingCondIlluminantType` = `D50`
