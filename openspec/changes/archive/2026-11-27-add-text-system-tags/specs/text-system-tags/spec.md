# text-system-tags Specification

## Purpose

Системные поля в text-выводе etng.

## ADDED Requirements

### Requirement: Selected system tags in text mode
При явном выборе системного тега etng SHALL печатать его в text-режиме.

#### Scenario: FileModifyDate
- **WHEN** запущен `etng -s -FileModifyDate file.jpg`
- **THEN** stdout содержит строку `FileModifyDate` с датой файла

### Requirement: No implicit system output
Без явного выбора системные поля в text-режиме MUST NOT добавляться.

#### Scenario: Обычный дамп
- **WHEN** запущен `etng file.jpg` без селекторов
- **THEN** вывод не меняется
