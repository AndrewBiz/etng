# Proposal

## Why

В phtools оставались writable-теги вне etng: `CountryCode`, `WorldRegion`,
`CodedCharacterSet`, `ImageUniqueID`.

## What Changes

- `CountryCode` → XMP `Iptc4xmpExt:LocationShownCountryCode`.
- `WorldRegion` → XMP `Iptc4xmpExt:LocationShownWorldRegion`.
- `CodedCharacterSet` → IPTC 1:90; `UTF8`/`UTF-8` → `ESC % G`.
- `ImageUniqueID` → ExifIFD `0xA420`, append-only.
- IPTC digest обновляется после изменения 1:90.

## Capabilities

### New Capabilities

- `phtools-extra-tags`: оставшиеся phtools-теги для JPEG.

## Impact

- `date_sidecars.rs`, `write.rs`, CLI/help, тесты.
