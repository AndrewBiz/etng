# Proposal

## Why

Функционал для текущих задач закрыт; известные расхождения должны быть
зафиксированы как осознанные ограничения, а не восприниматься как баги.

## What Changes

- `docs/divergences.md` переписан под v0.28.0: read/write matrix, Composite
  остатки, MakerNotes/preview offsets, phtools-ограничения, CLI/serializer.
- Убраны устаревшие утверждения («HEIC/CR3/MOV write not implemented»,
  «Composite:GPSAltitude missing»).
- Зафиксированы числа аудита: 1490 missing / 108 extra / 333 value diffs.

## Capabilities

### New Capabilities

- `known-limitations`: documented accepted gaps.

## Impact

- `docs/divergences.md`.
