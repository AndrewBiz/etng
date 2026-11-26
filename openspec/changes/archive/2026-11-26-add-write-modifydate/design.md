# Design

## Decisions

### D1. Append-only IFD0
Старые entries копируются байт-в-байт, новый `0x0132` добавляется в конец
нового IFD0. Значения и IFD1 остаются на прежних offset'ах.

### D2. TIFF header
Первая IFD в TIFF header (`bytes 4..8`) переводится на новый IFD0. Для JPEG
длина APP1 пересчитывается, файл перестраивается.

### D3. Существующий ModifyDate
Если тег есть, значение патчится на месте; новый IFD не создаётся.

### D4. AllDates
`DateTimeOriginal`/`CreateDate` обрабатываются ExifIFD-вставкой, затем
`ModifyDate` — IFD0-вставкой.

## Risks / Trade-offs

- **[Размер]** → +~100 байт на файл.
- **[Старый IFD0]** → остаётся орфанным блоком.
- **[XMP/IPTC digest]** → exiftool может предупредить; значения не меняются.

## Migration Plan

Аддитивно; откат — revert.
