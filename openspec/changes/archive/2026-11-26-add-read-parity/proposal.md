# Proposal

## Why

Анализ phtools/ftools-rs показал: для переименования по дате и чтения MWG-тегов
не хватает трёх вещей — вложенного XMP (`LocationShown`, `Collections`), PNG и
дат видео. На корпусе это 9 тегов на Sony JPEG, PNG-дата и даты двух MOV.

## What Changes

- XMP: нормализация префиксов `Iptc4xmpCore`/`Iptc4xmpExt` и разворачивание
  вложенных `LocationShown*` и `Collections` в плоские теги ExifTool.
- PNG: IHDR, eXIf (через общий EXIF-парсер), XMP из iTXt/tEXt, sRGB, tIME.
- MOV: ftyp, mvhd (CreateDate/ModifyDate/Duration), mdat.
- MTS/M2TS остаётся через fallback: H.264 SEI/M2TS — отдельный крупный этап.

## Capabilities

### New Capabilities

- `png-read`: PNG-метаданные.
- `mov-read`: базовые QuickTime/MOV-метаданные.

### Modified Capabilities

- `metadata-read`: вложенный XMP.

## Impact

- `crates/etng-core`: `xmp.rs`, `png.rs`, `mov.rs`, `sniff.rs`, `read.rs`.
- `base.png` — новый MIT-фикстур и снимок.
- На корпусе закрываются 9 XMP-тегов, PNG 32/32 и даты MOV.
