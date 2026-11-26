# Proposal

## Why

phtools `phfixdto` работает с MWG-набором тегов, а не только с датами. После
даты и XMP/IPTC остаются `City`, `Country`, `State`, `Location`, `Keywords`,
`Creator`, `Copyright`.

## What Changes

- Новые writable-теги: City, Country, State, Location, Keywords, Creator,
  Copyright.
- XMP: photoshop City/Country/State, Iptc4xmpCore:Location, dc:subject,
  dc:creator, dc:rights.
- IPTC: 2:90, 2:95, 2:101, 2:92, 2:25, 2:80, 2:116.
- EXIF: Artist (0x013B), Copyright (0x8298), append/update.
- Создание отсутствующих XMP/IPTC-пакетов.

## Capabilities

### New Capabilities

- `mwg-text-write`: запись MWG-текстовых тегов.

## Impact

- `crates/etng-core/src/{write.rs,date_sidecars.rs}`, CLI/help, тесты.
