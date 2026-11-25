# Spec Delta

## ADDED Requirements

### Requirement: Native Canon sub-tables in CR3
Система SHALL читать вложенные Canon-таблицы из `CMT3` нативно и MUST NOT требовать exiftool для AF/Lighting/Time/AFConfig-тегов при `--fallback=never`.

#### Scenario: Нативный AFConfig
- **WHEN** выполнен `etng -j --fallback=never -Canon:AFConfigTool photo.cr3`
- **THEN** значение равно `Case 1`, а не отсутствует

#### Scenario: Нативный часовой пояс
- **WHEN** выполнен `etng -j --fallback=never -Canon:TimeZone photo.cr3`
- **THEN** значение имеет вид `-08:00`
