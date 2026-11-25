# Proposal

## Why

CR3 сейчас не распознаётся и целиком уходит в fallback: JSON получается медленно, а `-b` для CR3 вообще отключён. Это единственный крупный формат современных Canon, который остался без нативного чтения.

## What Changes

- Детект CR3 по `ftyp` brand `crx `; `FileType=CR3`, MIME `image/x-canon-cr3`.
- Нативный разбор ISO-BMFF: `ftyp`, `mvhd`, треки (`tkhd`/`mdhd`/`hdlr`/`stsd`/`stts`/`stsz`/`co64`), `mdat`.
- Canon-метаданные из `CMT1` (IFD0), `CMT2` (ExifIFD), `CMT3` (MakerNotes Canon), `CMT4` (GPS).
- Реальные байты `Canon:ThumbnailImage` из `THMB` и `QuickTime:PreviewImage` из `PRVW`; `-b` работает для CR3.
- XMP из `uuid`-бокса (Rating и прочее).
- CR3 распознаётся нативно; `-b` и `--fallback=never` работают без exiftool.
- Пока не реализованы вложенные Canon-таблицы (CameraSettings/ShotInfo/FileInfo и др.), JSON и CSV в режиме `auto` заменяются полным выводом exiftool, чтобы не терять теги; после follow-up change fallback будет убран.
- Неподдерживаемые части (полный набор CanonCustom, массивы вроде `CompatibleBrands`) документируются как расхождения.

## Capabilities

### New Capabilities

- `cr3-read`: чтение CR3/ISO-BMFF: метаданные Canon, QuickTime/Track, встроенные изображения.

### Modified Capabilities

- `metadata-read`: CR3 добавляется в список поддерживаемых контейнеров.

## Impact

- `crates/etng-core`: новый `cr3`-парсер, общий BMFF-walker, расширение MakerNotes для CMT3, экспорт композитов.
- `crates/etng-cli`: ничего, кроме снятия fallback-пути для CR3.
- Тесты: CR3 в CLI, `-b` для превью/миниатюры, parity на `canon-r6.cr3`.
- Документация: `README.md`, `docs/divergences.md`, `PROVENANCE.md`.
