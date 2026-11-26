# Design

## Decisions

### D1. HEIC append
Новый EXIF item с недостающими датами добавляется как top-level `mdat` в конец
файла; `iloc` entry EXIF item переводится на новый offset. Существующие offset'ы
не меняются.

### D2. CR3 box growth
CMT2/CMT1 пересобираются с новым TIFF; размер box и `moov` увеличивается,
`stco`/`co64` сдвигаются на delta. Это сохраняет mdat и существующие item'ы.

### D3. IFD0 fallback
Если ExifIFD pointer отсутствует, DTO/CreateDate вставляются в IFD0
append-only relocation.

### D4. Границы
MOV-даты всегда в `mvhd`; вставка не требуется. MWG-текст в BMFF не входит.

## Risks / Trade-offs

- **[Размер]** → HEIC +~3 КБ, CR3 +~0.5 КБ.
- **[Старый item]** → HEIC оставляет старый EXIF item orphan.
- **[CR3 offsets]** → все chunk-offset'ы после вставки сдвигаются.

## Migration Plan

Аддитивно; откат — revert.
