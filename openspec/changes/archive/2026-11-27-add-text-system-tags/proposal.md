# Proposal

## Why

`exiftool -s -FileModifyDate` должен печатать системное поле. Сейчас etng
выводит системные поля только в JSON/CSV и в text-режиме они отсутствуют,
из-за чего ломается интеграция phtools (`phfixfmd`).

## What Changes

- В text-режиме при явном выборе тегов печатать совпавшие системные поля
  (`FileModifyDate`, `FileSize` и т.п.).
- Без явного выбора поведение не меняется.

## Capabilities

### New Capabilities

- `text-system-tags`: системные поля в text-выводе.

## Impact

- `crates/etng-cli/src/main.rs`, CLI-тесты.
