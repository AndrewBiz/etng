# Design

## Context

HEIC — ISO-BMFF: `ftyp`, `meta` (hdlr, pitm, iinf/iloc, iprp/ipco/ipma), `mdat`. Свойства изображения лежат в `ipco`, а `ipma` связывает их с item ID; primary item определяется `pitm`.

Проверено на корпусе: `ipma` version 0, associations — u8 с essential-битом (`& 0x7F`). Для primary item 49 индексы `[129,2,4,133,6]` → свойства 1 (colr), 2 (clli), 4 (ispe), 5 (irot), 6 (pixi). `irot` payload — один байт угла: 0 normal, 2 Rotate 180, 3 Rotate 90 CW.

## Decisions

### D1. Выбор свойств по `ipma`
Собираем карту item→property indices, берём primary из `pitm`. Для каждого kind ищем первое свойство, входящее в набор primary; если такого нет (hvcC), падаем на первое свойство этого kind.

### D2. `hvcC`
Поля читаются по фиксированным смещениям; compatibility flags печатаются как `Main Still Picture, Main 10, Main` по битам 30/29/28; level — `N (level N/30)`.

### D3. `colr`
Поддерживается `nclx`: primaries/transfer/matrix (2 → Unspecified, 1 → BT.709, 9 → BT.2020) и full-range flag; `prof` пропускается.

### D4. `mdat`
Размер и смещение считаются по заголовку бокса, а не по фактическому остатку файла.

## Risks / Trade-offs

- **[Строка вместо массива]** → `CompatibleBrands` в JSON строка; задокументировано.
- **[hvcC без привязки к item]** → для части файлов primary hvcC не входит в `ipma`; берём первый, возможны расхождения по level/profile.
- **[MetaImageSize]** → сложная структура `Meta`; пока не реализована.

## Migration Plan

Аддитивно. Откат — revert.

## Open Questions

Нет.
