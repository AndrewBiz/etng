# Proposal

## Why

iPhone MOV-файлы хранят локальное время создания в Apple-ключе
`com.apple.quicktime.creationdate`, а не только в UTC `mvhd`. ExifTool
показывает его как `Keys:CreationDate`; etng сейчас читает только
`QuickTime:CreateDate`, поэтому теряет локальную дату и offset.

## What Changes

- Разбор `moov/meta`:
  - `keys` — имена Apple-ключей (`mdta`);
  - `ilst` — значения.
- Новые теги:
  - `Keys:CreationDate` (`com.apple.quicktime.creationdate`);
  - `Keys:Make`, `Keys:Model`, `Keys:Software`.
- Нормализация Apple-даты `2021-02-22T17:28:32+0300` →
  `2021:02:22 17:28:32+03:00`.
- Unit-тест на синтетическом MOV-боксе.

## Capabilities

### New Capabilities

- `mov-apple-keys`: чтение Apple `Keys` metadata.

## Impact

- `crates/etng-core/src/mov.rs`, тесты; MOV-паритет с ExifTool.
