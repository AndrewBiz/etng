# Design

## Decisions

### D1. PATH shim
phtools вызывает `exiftool` из PATH; shim с таким именем перехватывает и
read, и write.

### D2. Read
Читающие вызовы передаются `etng` без изменений. Fallback на `/usr/bin/exiftool`
выключен по умолчанию, включается `ETNG_SHIM_FALLBACK=1`.

### D3. Write
`exiftool -@ script` парсится: строки-теги, файл, `-execute`. Поддерживаются
`-MWG:`, `-XMP:`, `-IPTC:`, GPS, `ImageUniqueID`, `Collections`,
`-FileModifyDate<DateTimeOriginal`. `-overwrite_original`, `-v0`,
`-ignoreMinorErrors` игнорируются.

### D4. Наблюдаемость
Все команды и неизвестные теги пишутся в `ETNG_SHIM_LOG`; strict-режим
показывает расхождения, fallback — для отладки.

## Risks / Trade-offs

- **[Неполный exiftool]** → shim поддерживает только теги phtools.
- **[Read-опции]** → неизвестные флаги приводят к ошибке в strict-режиме.
- **[Fallback]** → может скрыть пробелы; поэтому off by default.

## Migration Plan

Аддитивно; откат — удалить tools/.
