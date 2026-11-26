# Design

## Decisions

### D1. Карта тегов
| CLI | XMP | IPTC | EXIF |
| --- | --- | --- | --- |
| City | photoshop:City | 2:90 | — |
| Country | photoshop:Country | 2:101 | — |
| State | photoshop:State | 2:95 | — |
| Location | Iptc4xmpCore:Location | 2:92 | — |
| Keywords | dc:subject (Bag) | 2:25 | — |
| Creator | dc:creator (Seq) | 2:80 | Artist |
| Copyright | dc:rights | 2:116 | Copyright |

### D2. EXIF append
Существующий Artist/Copyright: если новое значение короче — пишется на месте;
если длиннее — значение дописывается в конец TIFF, entry обновляется. Новый тег
— append-only IFD0 relocation.

### D3. XMP arrays
Keywords/Creator заменяют существующий контейнер `rdf:Bag`/`rdf:Seq` одним
`rdf:li`; multi-value append пока не реализован.

### D4. IPTC
Keywords/Creator заменяют все записи dataset одним значением; остальные
scalar-датасеты обновляются/добавляются.

## Risks / Trade-offs

- **[Multi-value]** → списки схлопываются до одного значения.
- **[Кодировка]** → значения пишутся UTF-8; exiftool читает, встроенный reader
  показывает не-ASCII IPTC как binary (существующее ограничение чтения).
- **[IPTCDigest]** → не пересчитывается.

## Migration Plan

Аддитивно; откат — revert.
