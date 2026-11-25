# Proposal

## Why

JPEG-файлы Canon/Sony с MPF (Multi-Picture Format) содержат второе изображение (Large Thumbnail), которое exiftool отдаёт как `MPImage2:PreviewImage`. etng его не извлекал. Отдельно CR3 `THMB` обрезал хвост миниатюры на 5–7 байт, потому что доверял полю длины внутри бокса.

## What Changes

- Разбор APP2-сегмента `MPF\0`: IFD, `NumberOfImages`, `MPEntry`; абсолютные смещения считаются от начала TIFF MPF.
- Второе и последующие изображения выводятся как `MPImage2:PreviewImage` (и далее) с сырыми байтами.
- `-b -PreviewImage` работает для таких JPEG.
- CR3 `ThumbnailImage` берётся как `payload[16..]` (весь хвост бокса), а не по внутреннему полю длины.

## Capabilities

### New Capabilities

Нет.

### Modified Capabilities

- `metadata-read`: встроенные изображения JPEG (MPF) и точная длина CR3-миниатюры.

## Impact

- `crates/etng-core`: `jpeg.rs` (MPF), `read.rs` (вывод), `cr3.rs` (THMB).
- Тесты: `-b` на реальных JPEG/CR3, unit на MPF.
- Документация: `README.md`, `docs/divergences.md`.
