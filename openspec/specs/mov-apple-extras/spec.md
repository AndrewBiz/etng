# mov-apple-extras Specification

## Purpose

Дополнительные Apple-ключи MOV.

## Requirements

### Requirement: Apple GPS
etng SHALL читать `com.apple.quicktime.location.ISO6709` как
`Keys:GPSCoordinates` в формате ExifTool.

#### Scenario: ISO6709
- **WHEN** ключ содержит `+55.0132+037.4721+184.497/`
- **THEN** JSON содержит `Keys:GPSCoordinates` с deg/min/sec и высотой

### Requirement: Apple content identifier
etng SHALL читать `com.apple.quicktime.content.identifier` как
`Keys:ContentIdentifier`.

#### Scenario: UUID
- **WHEN** ключ содержит UUID
- **THEN** JSON содержит `Keys:ContentIdentifier` с этим UUID

### Requirement: ©day
etng SHALL читать `©day` как `ContentCreateDate` в группе `UserData` или
`ItemList`.

#### Scenario: UserData
- **WHEN** в `udta` есть `©day=2017-01-06T12:04:06+1000`
- **THEN** вывод содержит `UserData:ContentCreateDate` с
  `2017:01:06 12:04:06+10:00`
