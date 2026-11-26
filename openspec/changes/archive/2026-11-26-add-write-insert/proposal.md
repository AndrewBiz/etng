# Proposal

## Why

В корпусе phtools есть файлы без `DateTimeOriginal`/`CreateDate` (`tag_dto3`,
`4exif_createdate`, `6iptc_digitalcreationdate`, iPhone `no_dto_cd`). MVP умел
только перезаписывать существующие теги и падал на них.

## What Changes

- Вставка отсутствующих `DateTimeOriginal`/`CreateDate` в ExifIFD.
- Новый ExifIFD-блок добавляется в конец TIFF, указатель IFD0 обновляется;
  существующие значения и offset'ы не двигаются.
- Для JPEG обновляется длина APP1-сегмента; для TIFF файл дополняется.
- Пополнение корпуса фикстурами phtools (даты/теги) с provenance MIT.

## Capabilities

### New Capabilities

- `date-insertion`: вставка отсутствующих EXIF-дат.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/write.rs`, `xtask` (phtools исключён из compare),
  `tests/corpus/phtools/`.
- `ModifyDate` при отсутствии не вставляется.
