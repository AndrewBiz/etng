# heic-quicktime Specification

## Purpose

QuickTime/HEVC-метаданные HEIC: бренды, handler, свойства primary-изображения и `hvcC`.

## ADDED Requirements

### Requirement: HEIC file-type and handler metadata
Система SHALL выводить `QuickTime:MajorBrand`, `MinorVersion`, `CompatibleBrands` и `HandlerType` для HEIC и MUST брать их из `ftyp`/`meta`/`hdlr`.

#### Scenario: iPhone HEIC
- **WHEN** подан HEIC с brand `heic` и handler `pict`
- **THEN** MajorBrand содержит `High Efficiency Image Format HEVC still image (.HEIC)`, HandlerType — `Picture`

### Requirement: Primary image properties
Система SHALL выбирать свойства primary item через `ipma` и MUST выводить `ImageSpatialExtent`, `Rotation`, `ImagePixelDepth`, `AuxiliaryImageType` и цветовые параметры `colr`/`nclx`.

#### Scenario: Размеры primary
- **WHEN** primary item имеет `ispe` 3024×4032
- **THEN** `ImageSpatialExtent` = `3024x4032`, а не размеры thumbnail

#### Scenario: Поворот
- **WHEN** `irot` содержит угол 3
- **THEN** `Rotation` = `Rotate 90 CW`

### Requirement: HEVC configuration
Система SHALL разбирать `hvcC`: версию, profile space/tier/IDC, compatibility flags, constraints, level, chroma, глубину, frame rate и temporal layers.

#### Scenario: Main Still Picture
- **WHEN** `hvcC` содержит profile 3 и level 90
- **THEN** `GeneralProfileIDC` = `Main Still Picture`, `GeneralLevelIDC` = `90 (level 3.0)`

### Requirement: Media data location
Система MUST выводить `MediaDataSize` и `MediaDataOffset` из заголовка `mdat` даже при усечённом файле.

#### Scenario: mdat
- **WHEN** HEIC содержит `mdat`
- **THEN** MediaDataOffset указывает на первый байт данных, MediaDataSize — на объявленный размер
