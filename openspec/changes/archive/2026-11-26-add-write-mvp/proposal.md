# Proposal

## Why

phtools `phfixdto`/`phfixfmd` и сценарии ftools требуют записи дат. Без write
etng не может заменить их в ежедневном процессе, даже при полном чтении.

## What Changes

- ExifTool-совместимые присваивания `-TAG=VALUE` для дат:
  `DateTimeOriginal`, `CreateDate`, `ModifyDate`, `AllDates`, `FileModifyDate`.
- Запись существующих ASCII-полей в EXIF JPEG/TIFF-семейства на месте.
- Атомарная замена файла (temp + rename), сохранение прав.
- `FileModifyDate` через `set_modified` после замены.
- Ошибка без изменения файла, если тега нет.
- `-overwrite_original` принимается для совместимости (файл всегда заменяется).

## Capabilities

### New Capabilities

- `date-writing`: запись дат и FileModifyDate.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/write.rs`, CLI args/main.
- Тесты: EXIF/TIFF, CLI write, отсутствующий тег, FileModifyDate.
- HEIC/CR3/MOV write и вставка новых тегов — вне scope.
