# Proposal

## Why

Запись дат в Sony JPEG с MPF увеличивает sidecar-сегменты (APP13/XMP), сдвигает
MPImage2, но Sony MakerNote `PreviewImage` offset остаётся старым. ExifTool
выдаёт `[minor] Error reading PreviewImage`, а `Sony:PreviewImage` становится
пустым.

## What Changes

- После изменения JPEG sidecar'ов etng сдвигает Sony MakerNote
  `PreviewImage` offset (tag 0x2001) на delta длины файла.
- Работает и для date-write, и для text/GPS-write.
- Тест на synthetic MakerNote pattern + проверка реального `_DSC0109.JPG`.

## Capabilities

### New Capabilities

- `sony-preview-offset`: сохранение Sony preview при росте JPEG metadata.

## Impact

- `crates/etng-core/src/write.rs`, unit-тест.
