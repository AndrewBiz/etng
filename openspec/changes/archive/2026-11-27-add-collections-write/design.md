# Design

## Decisions

### D1. CLI
`-Collections='Name|URI'`, можно повторять. Без `|` URI пустой.

### D2. XMP
Namespace `http://www.metadataworkinggroup.com/schemas/collections/`;
свойство `mwg-coll:Collections` = `rdf:Bag`, элементы — `rdf:li`
с `rdf:parseType="Resource"` и двумя дочерними элементами.

### D3. Замена
При записи весь список Collections заменяется новым; старые структуры не
накапливаются. Остальные XMP-свойства сохраняются.

### D4. JPEG only
Для TIFF/BMFF XMP-write пока не поддержан; Collections пишется только в JPEG.

## Risks / Trade-offs

- **[Список]** → нельзя точечно удалить одну коллекцию; замена списка.
- **[URI]** → не валидируется как URL.
- **[Разделитель]** → `|` в имени/URI не экранируется.

## Migration Plan

Аддитивно; откат — revert.
