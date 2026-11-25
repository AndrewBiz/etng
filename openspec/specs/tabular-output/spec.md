# tabular-output Specification

## Purpose

Табличный CSV-вывод метаданных: одна строка на файл, стабильный набор колонок, пригодный для Excel, pandas и пакетных сводок.

## Requirements

### Requirement: CSV mode
Система SHALL поддерживать флаг `-csv`/`--csv`, формирующий CSV с первой колонкой `SourceFile`, и MUST отдавать приоритет CSV над JSON при одновременном указании `-csv` и `-j`.

#### Scenario: CSV вместо JSON
- **WHEN** выполнено `etng -csv -j -Make photo.jpg`
- **THEN** stdout содержит CSV-заголовок и строку, а не JSON-массив

#### Scenario: Заголовок и строка
- **WHEN** выполнено `etng -csv -Make photo.jpg`
- **THEN** первая строка — `SourceFile,Make`, вторая — путь к файлу и значение `Make`

### Requirement: Column composition and order
Система SHALL включать `SourceFile` первой колонкой; при выборе `-TAG`/`-GROUP:TAG` MUST выводить колонки в порядке аргументов с каноническим именем тега, а без выбора — MUST строить union тегов по всем входным файлам в алфавитном порядке.

#### Scenario: Порядок выбранных колонок
- **WHEN** выполнено `etng -csv -Model -Make photo.jpg`
- **THEN** заголовок равен `SourceFile,Model,Make`

#### Scenario: Канонический регистр
- **WHEN** выполнено `etng -csv -make photo.jpg`
- **THEN** колонка называется `Make`

#### Scenario: Union без выбора
- **WHEN** обрабатываются два файла с разными наборами тегов
- **THEN** заголовок содержит объединение тегов обоих файлов по алфавиту, а отсутствующие значения остаются пустыми ячейками

#### Scenario: Групповой префикс
- **WHEN** выполнено `etng -csv -G1 -Make photo.jpg`
- **THEN** колонка называется `IFD0:Make`

### Requirement: Cell values and formatting
Система SHALL форматировать ячейки как текстовые значения exiftool: с print-conversion, при `-n` — машинные значения; бинарные значения MUST выводиться как `base64:<data>` при `-b` и как заглушка `(Binary data N bytes, use -b option to extract)` без него.

#### Scenario: Машинные значения
- **WHEN** выполнено `etng -csv -n -Orientation -FileSize photo.jpg`
- **THEN** ячейки содержат `1` и размер файла в байтах

#### Scenario: Бинарное значение
- **WHEN** выполнено `etng -csv -b -ThumbnailImage photo.jpg`
- **THEN** ячейка начинается с `base64:/9j/`

### Requirement: CSV escaping
Система MUST заключать ячейку в двойные кавычки, если она содержит запятую, двойную кавычку, перевод строки или возврат каретки, и MUST удваивать внутренние двойные кавычки.

#### Scenario: Значение с запятой
- **WHEN** значение тега содержит `On, Fired`
- **THEN** в CSV оно записано как `"On, Fired"`

#### Scenario: Значение с кавычкой
- **WHEN** значение содержит двойную кавычку
- **THEN** она удвоена внутри quoted-ячейки, и CSV остаётся разбираемым

### Requirement: Deterministic tabular output
Для одинакового набора файлов и флагов система SHALL выдавать идентичные заголовок и строки при повторных запусках и MUST NOT зависеть от числа потоков.

#### Scenario: Повторный запуск
- **WHEN** одна и та же CSV-команда выполнена дважды
- **THEN** stdout побайтово идентичен, кроме полей времени доступа к файлам
