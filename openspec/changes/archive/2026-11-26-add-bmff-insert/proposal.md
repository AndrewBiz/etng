# Proposal

## Why

HEIC/CR3 умели только перезаписывать существующие даты. Если `DateTimeOriginal`
или `ModifyDate` отсутствовал, запись падала.

## What Changes

- HEIC: новый EXIF item дописывается в конец файла как `mdat`, `iloc`
  переводится на него; старые offset'ы не двигаются.
- CR3: CMT2/CMT1 растут, обновляются размеры `moov` и chunk-offset'ы
  `stco`/`co64`.
- Для отсутствующего DTO/CreateDate в IFD0 используется append-only IFD0.
- MOV вставка не нужна: `mvhd`-даты всегда есть.

## Capabilities

### New Capabilities

- `bmff-date-insert`: вставка отсутствующих EXIF-дат в HEIC/CR3.

## Impact

- `heic.rs` (`exif_info`, `set_exif_location`), `write.rs`.
