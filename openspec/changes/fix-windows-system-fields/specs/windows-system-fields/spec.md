# windows-system-fields Specification

## Purpose

Системные поля `etng` должны заполняться на Unix и Windows без Unix-only API.

## ADDED Requirements

### Requirement: Unix system fields

На Unix `FileInodeChangeDate`, `FilePermissions` и `FileMode` SHALL
заполняться из `MetadataExt` как раньше.

#### Scenario: Linux
- **WHEN** файл читается на Linux
- **THEN** вывод системных полей не меняется

### Requirement: Windows build

На Windows `etng-core` MUST собираться без `std::os::unix`.

#### Scenario: Cross-check
- **WHEN** выполняется `cargo check --target x86_64-pc-windows-gnu`
- **THEN** сборка проходит без ошибок и предупреждений

### Requirement: Windows fallback

На Windows `FileInodeChangeDate` SHALL браться из creation time,
`FilePermissions` — из readonly, `FileMode` SHALL быть `0`.

#### Scenario: Read-only file
- **WHEN** файл доступен только для чтения
- **THEN** `FilePermissions` равен `-r--r--r--`
