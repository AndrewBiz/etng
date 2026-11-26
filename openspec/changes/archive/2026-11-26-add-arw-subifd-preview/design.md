# Design

## Context

ARW — TIFF, где IFD0 содержит `0x014a` (SubIFDs), `0x0201`/`0x0202` (preview
offset/length), а также MakerNotes Sony. `kamadak-exif` отдаёт IFD0/ExifIFD/GPS,
но не SubIFD. Собственный `Tiff` уже используется для MakerNotes.

Проверено на `_DSC0426.ARW`: SubIFD по смещению из `0x014a` даёт 24 тега,
preview 145058/1118163 начинается с `FFD8`; SHA-256 `-b -PreviewImage`
совпадает с exiftool.

## Decisions

### D1. Только первый SubIFD
ExifTool выводит один набор `SubIFD:*`; при нескольких SubIFD берём первый,
чтобы не дублировать ключи.

### D2. Preview только для Sony
`0x0201`/`0x0202` в IFD0 у Sony — preview, но в обычном TIFF это thumbnail.
Поэтому извлечение включается только при `Make=SONY` и проверяется `FFD8`.

### D3. SR2SubIFD не трогаем
Блок `0x7200` зашифрован (ExifTool расшифровывает); без ключа это отдельный
большой проект.

## Risks / Trade-offs

- **[Дубли StripOffsets]** → SubIFD и preview могут пересекаться с IFD1; ключи
  в разных группах, как у exiftool.
- **[Sony preview не JPEG]** → байты не выводятся.

## Migration Plan

Аддитивно; откат — revert.

## Open Questions

Нет.
