# Proposal

## Why

Первый аудит не проверял rename-путь: все файлы были в стандартных именах.
Нужно проверить `phrename` на камерных именах и `phmove` с раскладкой
PHOTO/VIDEO/RAW.

## What Changes

- `corpus-audit.py` создаёт копии с камерными именами.
- Прогон `phls | phrename -a anb`, затем `phfixdto`.
- Отдельный прогон `phls | phmove -a target`.
- Проверка количества/категорий, отчёт и тайминги; CR2 помечается как
  неподдерживаемый phtools.

## Capabilities

### New Capabilities

- `corpus-audit-rename-move`: проверка rename/move.

## Impact

- `tools/corpus-audit.py`, README.
