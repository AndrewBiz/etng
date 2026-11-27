# Proposal

## Why

После изменения IPTC (даты/MWG) старый `Photoshop:IPTCDigest` оставался
несинхронным: exiftool выдавал `IPTCDigest is not current`. Для JPEG-коллекции
это лишний warning и риск, что XMP-aware приложения предпочтут устаревший IPTC.

## What Changes

- После каждого изменения IPTC пересчитывается MD5 IIM и обновляется
  Photoshop resource `0x0425`.
- Если ресурса не было — он добавляется.
- XMP не затрагивается; digest отражает именно новые IPTC-данные.

## Capabilities

### New Capabilities

- `iptc-digest`: синхронизация Photoshop IPTCDigest.

## Impact

- `date_sidecars.rs`, тест.
