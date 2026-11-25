# Tasks

## 1. Детект и BMFF

- [x] 1.1 Добавить `FileType::Cr3`, детект `ftyp` brand `crx `, MIME/extension
- [x] 1.2 BMFF-walker: боксы, поиск по kind внутри `uuid`, bounds-check

## 2. Canon-метаданные

- [x] 2.1 CMT1 → IFD0, CMT2 → ExifIFD (remap группы)
- [x] 2.2 CMT3 → Canon MakerNotes; layout «TIFF + first IFD»
- [x] 2.3 CMT4 → GPS с координатными print-конверсиями
- [x] 2.4 THMB → `Canon:ThumbnailImage`, PRVW → `QuickTime:PreviewImage`; `-b`

## 3. QuickTime

- [x] 3.1 `ftyp`: MajorBrand/MinorVersion/CompatibleBrands
- [x] 3.2 `mvhd`: даты, TimeScale, Duration, rate/volume, matrix, preview-поля, NextTrackID
- [x] 3.3 `mdat`: MediaDataSize/MediaDataOffset
- [x] 3.4 Треки: tkhd/mdhd/hdlr/stsd/stts/stsz/co64 → TrackN + JpgFromRaw
- [x] 3.5 XMP из uuid → Rating

## 4. Проверка

- [x] 4.1 CR3 читается без fallback; `-b -ThumbnailImage` = 12840, `-b -PreviewImage` = 237320
- [x] 4.2 `cargo xtask compare` — CR3 без расхождений вне задокументированных
- [x] 4.3 CLI/unit-тесты на детект, превью, усечённый mdat
- [x] 4.4 `cargo test --workspace`, `clippy -D warnings`, `fmt --check`
- [x] 4.5 Обновить README, divergences, PROVENANCE

## Workflow follow-up

- Archive the change after review.
- Verify `openspec list` is empty and specs validate with `--strict`.
