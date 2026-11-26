# Design

## Context

phtools/ftools-rs используют даты (`DateTimeOriginal`, `CreateDate`,
`ModifyDate`, `FileModifyDate`) и MWG-поля. Вложенный XMP не разворачивался,
PNG был Unknown, MOV уходил в fallback.

## Decisions

### D1. Нормализация XMP-префиксов
XML использует `Iptc4xmpCore`, `Iptc4xmpExt`, `mwg-coll`; таблицы — `iptcCore`,
`iptcExt`, `MWG::Collections`. Префикс маппится на таблицу, а для `mwg-coll`
id получает префикс `Collections`.

### D2. Вложенные LocationShown/Collections
При закрытии дочернего элемента проверяется стек: если внутри `LocationShown`,
локальное имя превращается в `LocationShownX`; для `Collections` id —
`CollectionsX`. Это даёт плоские имена, как у exiftool.

### D3. PNG eXIf
eXIf — обычный TIFF; используется существующий `exif::parse`. XMP читается из
iTXt с ключом `XML:com.adobe.xmp` (и tEXt). IHDR/sRGB/tIME разбираются вручную.

### D4. MOV — только заголовок
Читаются ftyp, mvhd и mdat; этого достаточно для дат/длительности в rename.
Треки и кодеки не трогаем. MTS не входит в change.

## Risks / Trade-offs

- **[CompatibleBrands — строка]** → известное расхождение с JSON-массивом.
- **[MOV-даты UTC]** → exiftool печатает без зоны; формат совпадает.
- **[MTS]** → остаётся fallback.

## Migration Plan

Аддитивно; откат — revert.

## Open Questions

Нет.
