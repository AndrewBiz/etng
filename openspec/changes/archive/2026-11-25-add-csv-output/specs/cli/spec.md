# Spec Delta

## ADDED Requirements

### Requirement: Tag exclusion
Система SHALL поддерживать повторяемые флаги `-x TAG`, `-x GROUP:TAG` и `--exclude`, исключающие указанные теги из всех режимов вывода; сопоставление MUST быть регистронезависимым, а исключение MUST иметь приоритет над выбором `-TAG`.

#### Scenario: Исключение в JSON
- **WHEN** выполнено `etng -j -x Make photo.jpg`
- **THEN** поле `Make` отсутствует, остальные поля выводятся как обычно

#### Scenario: Исключение группы
- **WHEN** выполнено `etng -j -x IFD0:Make photo.jpg`
- **THEN** исключается только `Make` из группы `IFD0`

#### Scenario: Исключение сильнее выбора
- **WHEN** выполнено `etng -j -Make -Model -x Model photo.jpg`
- **THEN** в выводе остаётся только `Make`

#### Scenario: Исключение колонки CSV
- **WHEN** выполнено `etng -csv -Make -Model -x Model photo.jpg`
- **THEN** заголовок содержит `SourceFile,Make`, колонки `Model` нет

#### Scenario: Неизвестное имя исключения
- **WHEN** указано `-x NoSuchTag`
- **THEN** команда не завершается ошибкой использования, исключение просто не совпадает ни с одним тегом
