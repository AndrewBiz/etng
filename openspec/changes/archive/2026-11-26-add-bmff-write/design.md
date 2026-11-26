# Design

## Decisions

### D1. Только существующие поля
HEIC/CR3/MOV не поддерживают вставку: новые даты/IFD меняют длины и offset'ы
BMFF. MVP перезаписывает 19-байтные значения на месте.

### D2. HEIC
`heic::extract_exif` возвращает TIFF payload; сохраняем его offset в файле,
копируем, применяем `apply_exif`, копируем обратно.

### D3. CR3
CMT2 содержит `DateTimeOriginal`/`CreateDate`, CMT1 — `ModifyDate`. Каждая дата
ищется в обоих payload'ах; первый найденный обновляется.

### D4. mvhd
`moov/mvhd` версии 0/1: 32/64-битные секунды от 1904. Для MOV — UTC, для CR3 —
локальная зона (совместимо с exiftool-выводом).

## Risks / Trade-offs

- **[Нет вставки]** → отсутствующая дата возвращает ошибку.
- **[Часовой пояс]** → MOV UTC, CR3 local; проверено exiftool.
- **[BMFF offsets]** → in-place patch не меняет размеры.

## Migration Plan

Аддитивно; откат — revert.
