# Design

## Context

Track4 — `meta`-трек с сэмплом `CTMD` (117100 байт у R50). Внутри несколько
вложенных TIFF-блоков Canon MakerNote; первый содержит ColorData11, другие —
CameraSettings/LensInfo. ExifTool выводит их как `Track4:*`.

FilterInfo `0x4024` — директория фильтров: count + записи (filter number, size,
parameter count, params). FocusBracketing `0x4053` и LevelInfo `0x4059` —
`int32s`-таблицы.

## Decisions

### D1. Поиск TIFF-блоков, а не MakerNote по 0x927c
В CTMD MakerNote лежит как самостоятельный TIFF (ColorData), а не через
ExifIFD-тег 0x927c. Поэтому сканируем все `II*\0`/`MM\0*`, парсим каждый
`parse_canon_at` и дедуплицируем по имени.

### D2. Группа Track4
Теги переименовываются в `Track4` после разбора; исходные `MakerNotes:Canon`
не сохраняются, чтобы не дублировать.

### D3. Composite только для CR3
Новые Composite (ISO/Lens/Rotation/WB) включаются при наличии `Track4`,
иначе JPEG-паритет ломается (`Composite:ISO` у exiftool там нет).

### D4. Filter/Level conversions
Filter: `-1` → `Off`, иначе `On (N)`; Orientation 0/1 → Horizontal/Vertical.
Level: Roll = -val/10 с переходом через 3600, Pitch = val/10, фокусные — `/10`
и `N mm`.

## Risks / Trade-offs

- **[Дубли Track4]** → BTreeSet по имени.
- **[Неполный CTMD]** → если TIFF не найден, Track4 ограничивается метаданными
  трека.
- **[Остаток]** → CanonCustom (3) и сложные Composite (19) пока не считаются.

## Migration Plan

Аддитивно; откат — revert.

## Open Questions

Нет.
