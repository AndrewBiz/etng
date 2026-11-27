# sony-preview-offset Specification

## Purpose

Сохранение Sony PreviewImage при изменении размера JPEG metadata.

## ADDED Requirements

### Requirement: Shift Sony preview offset
Если sidecar-сегменты выросли, etng SHALL сдвигать Sony MakerNote
`PreviewImage` offset на delta.

#### Scenario: MPF JPEG
- **WHEN** `_DSC0109.JPG` получает новые даты
- **THEN** `Sony:PreviewImage` читается, `exiftool -validate` — OK

### Requirement: No-op without Sony pattern
Без Sony MakerNote/MPF etng MUST NOT менять посторонние offset'ы.

#### Scenario: Обычный JPEG
- **WHEN** файл без `SONY DSC`
- **THEN** запись завершается как раньше
