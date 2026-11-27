# Proposal

## Why

PNG в корпусе читается полностью, но запись дат отвечала
`date writing is not supported for PNG`; `phfixdto` на PNG не работал.

## What Changes

- Запись дат в PNG `eXIf` chunk.
- Обновление существующего `eXIf` с ExifIFD pointer.
- Создание `eXIf` (IFD0 + ExifIFD) при его отсутствии и при отсутствии
  ExifIFD pointer, с сохранением существующих IFD0-тегов.
- Пересчёт длины/CRC чанков, вставка перед `IDAT`.

## Capabilities

### New Capabilities

- `png-date-write`: запись DateTimeOriginal/CreateDate/ModifyDate в PNG.

## Impact

- `crates/etng-core/src/png.rs`, `write.rs`, CLI-тесты, README.
