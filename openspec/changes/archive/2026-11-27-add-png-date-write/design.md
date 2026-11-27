# Design

## D1. Container
`eXIf` содержит raw TIFF (без `Exif\0\0`). При наличии чанк заменяется;
при отсутствии вставляется перед первым `IDAT`. Длины big-endian, CRC32 по
типу и payload.

## D2. Обновление
Сначала `apply_exif` по TIFF. Если ExifIFD pointer есть, отсутствующие даты
добавляются `insert_dates`. Если pointer нет, TIFF перестраивается:
сохраняются существующие IFD0-записи (включая GPS pointer), добавляется
ExifIFD с DTO/CreateDate и IFD0 ModifyDate.

## D3. Новый eXIf
Строится минимальный little-endian TIFF: IFD0 (ModifyDate + ExifIFD pointer),
ExifIFD (DTO/CreateDate). Значения — 20-байтовые ASCII.

## Risks
Перестройка без ExifIFD дублирует out-of-line значения, но сохраняет старые
offset'ы; проверено на `base.png` и реальном PNG.
