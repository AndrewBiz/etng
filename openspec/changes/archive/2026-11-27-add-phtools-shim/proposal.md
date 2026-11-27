# Proposal

## Why

Нужен воспроизводимый способ прогнать phtools через etng, а не через
настоящий exiftool: чтение и запись.

## What Changes

- `tools/exiftool-shim/exiftool` — shim для PATH:
  - read → etng;
  - `exiftool -@ script` → etng-ассignments;
  - MWG/XMP/IPTC/GPS/Collections и `-FileModifyDate<DateTimeOriginal`.
- `tools/phtools-integration.sh` — сборка etng, PATH shim, запуск cucumber.
- Smoke-тесты shim без Ruby/exiftool.
- CI запускает smoke-тесты.

## Capabilities

### New Capabilities

- `phtools-integration`: воспроизводимый прогон phtools с etng.

## Impact

- `tools/`, CI, README.
