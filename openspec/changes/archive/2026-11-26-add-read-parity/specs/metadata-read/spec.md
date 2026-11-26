# metadata-read Delta

## ADDED Requirements

### Requirement: XMP nested structures
Система SHALL разворачивать вложенные `Iptc4xmpExt:LocationShown` и
`mwg-coll:Collections` в плоские теги ExifTool и MUST нормализовать префиксы
`Iptc4xmpCore`/`Iptc4xmpExt` к таблицам `iptcCore`/`iptcExt`.

#### Scenario: LocationShown
- **WHEN** XMP содержит `LocationShown` с City=Brugge и CountryCode=BE
- **THEN** вывод содержит `XMP-iptcExt:LocationShownCity` и
  `LocationShownCountryCode`

#### Scenario: Collections
- **WHEN** XMP содержит `mwg-coll:Collections` с CollectionName
- **THEN** `XMP-mwg-coll:CollectionName` присутствует

### Requirement: XMP core namespace
Система MUST читать `Iptc4xmpCore:Location` как `XMP-iptcCore:Location`.

#### Scenario: Location
- **WHEN** XMP содержит `Iptc4xmpCore:Location=Grote Markt`
- **THEN** `XMP-iptcCore:Location` = `Grote Markt`
