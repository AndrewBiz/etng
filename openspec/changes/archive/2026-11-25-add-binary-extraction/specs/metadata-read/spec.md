# Spec Delta

## ADDED Requirements

### Requirement: Embedded image payloads
Система SHALL материализовать байты встроенных изображений: миниатюру IFD1 как `IFD1:ThumbnailImage` для JPEG, TIFF и HEIC и превью CR2 как `IFD0:PreviewImage`, MUST сохранять их длину и MUST NOT подменять содержимое нулями.

#### Scenario: JPEG с миниатюрой
- **WHEN** JPEG содержит IFD1 с корректными `ThumbnailOffset` и `ThumbnailLength`
- **THEN** в результате присутствует `IFD1:ThumbnailImage` длиной `ThumbnailLength`, а обычный JSON показывает заглушку `(Binary data N bytes, use -b option to extract)`

#### Scenario: CR2 с превью
- **WHEN** CR2 содержит `PreviewImageStart` и `PreviewImageLength` в пределах файла
- **THEN** `IFD0:PreviewImage` содержит реальные байты превью, а не нулевой заполнитель

#### Scenario: Смещение вне файла
- **WHEN** смещение или длина встроенного изображения выходят за границы файла
- **THEN** тег изображения не добавляется, чтение остальных метаданных продолжается без паники
