# known-limitations Specification

## Purpose

Явно перечисленные принятые ограничения etng.

## Requirements

### Requirement: Document accepted gaps
Проект SHALL вести `docs/divergences.md` как список осознанных ограничений с
указанием версии и затронутых форматов.

#### Scenario: Write matrix
- **WHEN** пользователь проверяет поддержку записи
- **THEN** документ показывает JPEG/TIFF/PNG/HEIC/CR3/MOV и отсутствие MTS

### Requirement: No stale implemented claims
Документ MUST NOT перечислять уже реализованные возможности как отсутствующие.

#### Scenario: PNG dates
- **WHEN** открыт раздел write mode
- **THEN** PNG указан как поддерживаемый, а не как gap
