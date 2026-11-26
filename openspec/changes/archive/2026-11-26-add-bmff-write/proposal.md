# Proposal

## Why

После JPEG/TIFF оставались HEIC, CR3 и MOV. Для них можно перезаписывать
существующие даты без перестройки контейнера.

## What Changes

- HEIC: EXIF item через `iloc`/`extract_exif`, in-place patch.
- CR3: CMT2 (DTO/CreateDate) и CMT1 (ModifyDate), in-place patch.
- MOV/CR3: `moov/mvhd` CreateDate/ModifyDate.
- Для CR3 QuickTime-даты пишутся в локальной зоне, для MOV — в UTC.
- Вставка отсутствующих дат и MWG-текст в BMFF не входят.

## Capabilities

### New Capabilities

- `bmff-date-write`: запись дат в HEIC/CR3/MOV.

## Impact

- `write.rs`, public `cr3::exif_payloads`, CLI-тесты.
