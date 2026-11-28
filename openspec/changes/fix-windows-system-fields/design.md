# Design

## D1. Границы

Меняется только `system_fields`; формат JSON/CSV не трогаем. На Unix
поведение байт-в-байт прежнее.

## D2. Windows fallback

- `file_inode_change_date` — `format_datetime(metadata.created())`; на Windows
  это creation time, ближайший аналог inode-change.
- `file_permissions` — `-r--r--r--` для readonly, иначе `-rw-rw-rw-`.
- `file_mode` — `0`.

## D3. Imports

`MetadataExt`, `format_unix`, `format_permissions` импортируются только под
`cfg(unix)`, иначе Windows-сборка с `-D warnings` падает на unused imports.

## Risks

- Windows-значения `FilePermissions` не совпадают с exiftool; это осознанный
  fallback, ftng их не использует.
- Проверка выполнения Windows-бинаря требует Wine/Windows; PE-формат
  проверяется `file`, поведение — кросс-компиляцией и Linux-тестами.
