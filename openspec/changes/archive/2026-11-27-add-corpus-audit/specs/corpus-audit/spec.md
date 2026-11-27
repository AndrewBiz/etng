# corpus-audit Specification

## Purpose

Аудит etng на локальном корпусе.

## ADDED Requirements

### Requirement: No original mutation
Аудит SHALL работать только с копиями файлов.

#### Scenario: Запись
- **WHEN** запускается `phfixdto`
- **THEN** изменяется только временная копия

### Requirement: Compare with exiftool
Аудит SHALL сравнивать `etng -j -G1` и `exiftool -j -G1` и сообщать
missing/extra/value-diff.

#### Scenario: Отчёт
- **WHEN** аудит завершён
- **THEN** JSON-отчёт содержит сводку по каждому файлу и top-теги

### Requirement: No recursive fallback
Shim MUST запускать etng с `--fallback=never`.

#### Scenario: CR3
- **WHEN** etng читает CR3, для которого нужен fallback
- **THEN** он не вызывает shim повторно
