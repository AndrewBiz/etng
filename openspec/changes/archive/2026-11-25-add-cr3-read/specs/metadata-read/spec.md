# Spec Delta

## ADDED Requirements

### Requirement: CR3 container
Система SHALL читать метаданные из CR3 (ISO-BMFF) наравне с остальными поддерживаемыми контейнерами и MUST распознавать CR3 нативно; режим `--fallback=never` MUST возвращать нативный набор без ошибки Unknown file type.

#### Scenario: CR3 без fallback
- **WHEN** выполнен `etng -j photo.cr3` с `--fallback=never`
- **THEN** вывод содержит метаданные CR3 и не содержит ошибки Unknown file type
