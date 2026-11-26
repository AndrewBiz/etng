# Design

## Decisions

### D1. Минимальные пакеты
XMP создаётся как `<x:xmpmeta><rdf:RDF>` с одним `rdf:Description` и нужными
namespace; IPTC — как один Photoshop resource `0x0404` с IIM record 2.

### D2. Добавление недостающего
В существующий XMP свойство добавляется в `rdf:Description` с его namespace
или в новый description. В существующий IPTC dataset добавляется в конец
record 2; порядок остальных записей сохраняется.

### D3. Вставка сегментов
Новые APP1 XMP / APP13 Photoshop вставляются сразу после SOI, длина сегмента
пересчитывается.

### D4. Время IPTC
Если TimeCreated уже есть — сохраняется его смещение; если нет — `+0000`.

## Risks / Trade-offs

- **[Валидность XMP]** → минимальный packet без padding; exiftool читает.
- **[IPTCDigest]** → digest не пересчитывается; exiftool предупреждает.
- **[Размер]** → +~0.5 КБ на файл.

## Migration Plan

Аддитивно; откат — revert.
