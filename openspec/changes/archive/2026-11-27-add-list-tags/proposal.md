# Proposal

## Why

`MiniExiftool` строит кэш имён тегов через `exiftool -list`/`-listw`. Без этих
опций phtools на чистой машине получает пустой кэш и `phgettags --full_dump`
печатает один `nil`-ключ. Локально это скрывалось старым pstore-кэшем.

## What Changes

- `etng -list` и `etng -listw` печатают имена тегов из встроенных таблиц.
- Формат совместим с парсером ExifTool/MiniExiftool: строки с отступом,
  имена через пробел.

## Capabilities

### New Capabilities

- `tag-list`: вывод списка тегов.

## Impact

- `crates/etng-cli`, тесты; phtools CI.
