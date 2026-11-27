# Design

## Decisions

### D1. Где digest
Стандартный Photoshop IRB resource `0x0425` в APP13, 16 сырых байт MD5 (не hex,
не base64). `LegacyIPTCDigest` в XMP не используется.

### D2. Когда
Только когда IPTC реально изменился (`update_iptc`/`update_iptc_text` вернули
новый буфер). XMP-only изменения digest не трогают.

### D3. Как
`refresh_iptc_digest`: найти APP13 и 0x0404 IIM, посчитать MD5, пересобрать
8BIM-блок, заменить/добавить 0x0425, обновить длину APP13.

## Risks / Trade-offs

- **[Лишний ресурс]** → добавляется, если его не было; это соответствует MWG.
- **[Старые XMP]** → digest синхронизирует IPTC и XMP после наших правок.

## Migration Plan

Аддитивно; откат — revert.
