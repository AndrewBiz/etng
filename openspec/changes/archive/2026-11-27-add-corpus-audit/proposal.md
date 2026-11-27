# Proposal

## Why

Нужен воспроизводимый аудит etng на реальном корпусе: сравнение с exiftool,
прогон phtools на копиях, проверка валидности и замер времени. Ручной прогон
уже выявил рекурсию shim → etng → shim на CR3.

## What Changes

- `tools/corpus-audit.py`: копии, JSON-сравнение etng/exiftool, phls/
  phrename/phfixdto через shim, validate, тайминги, отчёт.
- Shim передаёт etng `--fallback=never`, чтобы etng не вызывал shim как
  exiftool при auto-fallback.
- CI: smoke-тесты shim уже есть; аудит запускается вручную на локальном
  корпусе (реальные файлы не коммитятся).

## Capabilities

### New Capabilities

- `corpus-audit`: локальный аудит корпуса и тайминги.

## Impact

- `tools/`, shim tests, README.
