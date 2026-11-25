# binary-output Specification

## Purpose

Извлечение и вывод сырых значений метаданных: встроенных изображений и произвольных бинарных блоков, в текстовом и JSON-режимах, совместимо с `exiftool -b`.

## ADDED Requirements

### Requirement: Binary mode flag
Система SHALL поддерживать флаг `-b`/`--binary` в текстовом и JSON-режимах и MUST NOT менять набор, порядок или форматирование полей, к которым флаг не относится.

#### Scenario: Флаг в JSON-режиме
- **WHEN** запущено `etng -j -b -Make photo.jpg`
- **THEN** вывод остаётся JSON-массивом с `SourceFile` и `"Make": "Canon"`

#### Scenario: Флаг без выбора тегов
- **WHEN** запущено `etng -b photo.jpg`
- **THEN** в stdout последовательно пишутся системные поля и все теги без ключей и разделителей

### Requirement: Raw value stream
В текстовом режиме с `-b` система SHALL писать значения в stdout как последовательность байтов без переводов строк и разделителей, MUST выводить скалярные значения без print-conversion и MUST выводить бинарные теги их исходными байтами.

#### Scenario: Скалярное значение
- **WHEN** выполнено `etng -b -Make photo.jpg`
- **THEN** stdout содержит ровно `Canon` без завершающего перевода строки

#### Scenario: Число без print-conversion
- **WHEN** выполнено `etng -b -Orientation photo.jpg`
- **THEN** stdout содержит `1`, а не `Horizontal (normal)`

#### Scenario: Порядок выбранных тегов
- **WHEN** выполнено `etng -b -Model -Make photo.jpg`
- **THEN** значения идут в порядке аргументов: сначала `Model`, затем `Make`

#### Scenario: Несколько файлов
- **WHEN** команда получает два файла
- **THEN** значения второго файла пишутся сразу после значений первого, без разделителя

### Requirement: Base64 in JSON mode
В JSON-режиме с `-b` система SHALL представлять каждое бинарное значение строкой `"base64:<standard-base64>"`, а непустые небинарные значения MUST оставлять в обычном JSON-представлении.

#### Scenario: Миниатюра в JSON
- **WHEN** выполнено `etng -j -b -ThumbnailImage photo.jpg`
- **THEN** значение поля начинается с `base64:/9j/` и декодируется в JPEG

#### Scenario: Текст в JSON
- **WHEN** выполнено `etng -j -b -Make photo.jpg`
- **THEN** поле `Make` остаётся строкой `Canon`

### Requirement: Embedded image extraction
Система SHALL извлекать байты встроенной миниатюры IFD1 и превью CR2 из входного файла и MUST отдавать их в режиме `-b` без искажения длины.

#### Scenario: Миниатюра JPEG
- **WHEN** JPEG содержит IFD1 с `ThumbnailOffset` и `ThumbnailLength`
- **THEN** `-b -ThumbnailImage` возвращает ровно `ThumbnailLength` байт, начинающихся с `FFD8`

#### Scenario: Превью CR2
- **WHEN** CR2 содержит `PreviewImageStart` и `PreviewImageLength`
- **THEN** `-b -PreviewImage` возвращает реальные байты файла, а не нули

### Requirement: Output hygiene
В режиме `-b` система MUST писать в stdout только значения, MUST направлять ошибки и предупреждения в stderr и MUST сохранять прежние коды возврата.

#### Scenario: Ошибка чтения файла
- **WHEN** один из файлов не читается
- **THEN** сообщение уходит в stderr, код возврата равен 1, а stdout не содержит текста ошибки

#### Scenario: Fallback
- **WHEN** файл требует делегирования системному exiftool
- **THEN** в режиме `-b` делегирование не выполняется, а причина сообщается в stderr
