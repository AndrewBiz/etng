# Design

## Context

MPF-сегмент JPEG: APP2 (`FFE2`) с payload `MPF\0` + TIFF-заголовок. В IFD0:
`0xB001 NumberOfImages`, `0xB002 MPEntry` (по 16 байт: attr, size, offset, deps). Смещение в MPEntry отсчитывается от начала MPF TIFF (позиция после `MPF\0`), поэтому абсолютный офсет = `tiff_start + entry.offset`.

Проверено на корпусе:

- Canon EOS R50: MPImage2 590 042 байта, MPImage2 651 380;
- Sony A7: MPImage2 437 525;
- CR3 `THMB`: exiftool отдаёт `box_size - 24`, включая нулевой padding, а не внутреннее поле длины.

## Decisions

### D1. Сканер сегментов JPEG
Идём по маркерам, пропускаем standalone-маркеры, читаем длину APP2; при `MPF\0` парсим TIFF.

### D2. Абсолютные смещения
`tiff_start` = позиция после `MPF\0`; `abs = tiff_start + offset`. Проверяем границы и сигнатуру `FFD8`.

### D3. Группы
`MPImage1..4` — статические имена; primary (index 0) не выводим, остальные — `PreviewImage`.

### D4. THMB
Длина = `payload.len() - 16`; это совпадает с `box_size - 24` и покрывает padding. Для JPEG-миниатюр IFD1 логика не меняется.

## Risks / Trade-offs

- **[MPF с offset относительно файла]** → по спецификации offset относителен MPF TIFF; проверено на трёх файлах.
- **[Sony:PreviewImage]** → отдельный trailer Sony не реализован, но MPImage2 совпадает побайтово.
- **[Более 4 изображений]** → поддерживаются первые четыре (MPImage1..4).

## Migration Plan

Аддитивно. Откат — revert.

## Open Questions

Нет.
