# Proposal

## Why

На ARW Sony exiftool выводит 24 тега группы `SubIFD` (размеры RAW, CFA, коррекции,
strip-параметры) и `IFD0:PreviewImage*`, а etng не выводит ничего из этого:
`kamadak-exif` не спускается в SubIFD, а preview-байты не извлекались.

## What Changes

- Разбор первого SubIFD через собственный TIFF-walker: все известные теги
  `Exif::Main` в группе `SubIFD`.
- Sony ARW preview: `IFD0:PreviewImageStart`, `PreviewImageLength`,
  `PreviewImage` из `0x0201`/`0x0202`, с проверкой JPEG-маркера.
- `-b -PreviewImage` для ARW отдаёт те же байты, что exiftool.
- SR2SubIFD (зашифрованный блок) и 164 Sony-тега остаются вне scope.

## Capabilities

### New Capabilities

- `arw-subifd`: SubIFD и preview для Sony ARW.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/exif.rs`: `parse_subifds`, `sony_preview`.
- `makernotes::entry_value` становится `pub(crate)`.
- Тесты: синтетический TIFF с SubIFD и preview.
