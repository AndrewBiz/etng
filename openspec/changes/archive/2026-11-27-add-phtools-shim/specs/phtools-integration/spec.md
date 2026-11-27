# phtools-integration Specification

## Purpose

Прогон phtools через etng.

## ADDED Requirements

### Requirement: Read shim
Shim SHALL передавать read-вызовы в etng.

#### Scenario: phgettags
- **WHEN** phtools вызывает exiftool для чтения
- **THEN** используется etng, команда пишется в лог

### Requirement: Write script translation
Shim SHALL переводить `exiftool -@ script` в вызовы etng, поддерживая теги
phtools.

#### Scenario: phfixdto
- **WHEN** скрипт содержит `-MWG:DateTimeOriginal=` и `-FileModifyDate<...`
- **THEN** etng получает `-DateTimeOriginal=` и `-FileModifyDate=`

### Requirement: Strict by default
Read fallback на exiftool MUST быть выключен по умолчанию.

#### Scenario: Неподдерживаемая опция
- **WHEN** etng не понимает read-опцию
- **THEN** shim возвращает ошибку, а не подменяет результат
