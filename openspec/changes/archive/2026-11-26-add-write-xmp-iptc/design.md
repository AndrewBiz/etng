# Design

## Context

JPEG может содержать три независимых хранилища дат: EXIF (TIFF), XMP (APP1
`http://ns.adobe.com/xap/1.0/`) и IPTC (APP13 `Photoshop 3.0`). Это разные
сегменты, поэтому EXIF-вставка их не затрагивает.

## Decisions

### D1. Только существующие поля
XMP-элемент/атрибут и IPTC dataset обновляются, если уже есть. Новый XMP-пакет
или IPTC-ресурс не создаётся: это отдельная задача с namespace/padding.

### D2. Форматы
XMP — ISO `YYYY-MM-DDTHH:MM:SS`. IPTC DateCreated — `YYYYMMDD`, TimeCreated —
`HHMMSS` + существующее смещение (`+0200`); DigitalCreation — те же правила.

### D3. Перестройка сегментов
При изменении длины XMP/APP13 пересчитывается длина сегмента; остальные
сегменты JPEG не двигаются семантически.

### D4. Атомарность
EXIF-патч, вставка и XMP/IPTC-обновление выполняются в памяти, затем один
atomic temp+rename.

## Risks / Trade-offs

- **[XMP padding]** → пересчёт длины может уменьшить padding; XML остаётся
  валидным.
- **[IPTCDigest]** → exiftool предупреждает о несинхронном digest, значения
  не меняются.
- **[Отсутствующие пакеты]** → не создаются; дата останется только в EXIF.

## Migration Plan

Аддитивно; откат — revert.
