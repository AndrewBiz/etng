# Proposal

## Why

ICC-профили — 26–39 тегов на JPEG/HEIC (`ICC-header`, `ICC_Profile`, `ICC-view`,
`ICC-meas`), которых etng не выводил совсем. Это цветовое пространство,
white point, матрицы каналов, TRC и условия просмотра — важные метаданные.

## What Changes

- Новый модуль `icc`: разбор 128-байтного заголовка и tag table.
- Извлечение профиля: JPEG APP2 `ICC_PROFILE`, TIFF `0x8773`, HEIC `colr/prof`.
- Теги: 16 header, ProfileDescription/Copyright, white/black point, RGB-матрицы,
  ChromaticAdaptation, TRC (bytes), Technology, view/meas-условия.
- Поддержка `desc`, `text`, `mluc`, `utf8` строк.
- `-b -RedTRC` отдаёт сырые байты.

## Capabilities

### New Capabilities

- `icc-profiles`: ICC-профили в JPEG/TIFF/HEIC.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/icc.rs`, вызовы из `read.rs`.
- `xtask compare`: ICC больше не «known vendor», сравнение стало строгим.
- Тест на JPEG и HEIC.
