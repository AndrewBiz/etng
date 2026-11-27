# Design

## Decisions

### D1. XMP iptcExt
`CountryCode`/`WorldRegion` — scalar XMP-свойства в namespace
`http://iptc.org/std/Iptc4xmpExt/2008-02-29/`; создаются при отсутствии пакета.

### D2. IPTC 1:90
`CodedCharacterSet` — запись record 1 dataset 90; `UTF8`/`UTF-8` кодируется
как `\x1b%G`, другие значения — как есть. После изменения вызывается
`refresh_iptc_digest`.

### D3. ImageUniqueID
ExifIFD `0xA420`, ASCII; существующее значение патчится, отсутствующее
вставляется append-only relocation ExifIFD и обновлением указателя `0x8769`.

## Risks / Trade-offs

- **[CodedCharacterSet]** → не-UTF8 значения не валидируются.
- **[ImageUniqueID]** → нужен существующий ExifIFD; при его отсутствии ошибка.

## Migration Plan

Аддитивно; откат — revert.
