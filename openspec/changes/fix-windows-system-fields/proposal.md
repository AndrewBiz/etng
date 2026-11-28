# Proposal

## Why

`etng-core` не собирается под `x86_64-pc-windows-gnu`: `system_fields`
безусловно использует Unix-only `MetadataExt` (`ctime`, `ctime_nsec`, `mode`).
Без этого нельзя выпустить Windows-бинарь `ftng`.

## What Changes

- Unix-ветка сохраняет текущие `FileInodeChangeDate`, `FilePermissions`,
  `FileMode`.
- Windows-ветка не использует Unix API: `FileInodeChangeDate` из времени
  создания, права из readonly, `FileMode = 0`.
- Linux-вывод и все существующие тесты/сравнения не меняются.

## Capabilities

### New Capabilities

- `windows-system-fields`: кросс-платформенные системные поля.

## Impact

- `crates/etng-core/src/read.rs`
- версия 0.28.2; зависимость `ftng` обновляется отдельно.
