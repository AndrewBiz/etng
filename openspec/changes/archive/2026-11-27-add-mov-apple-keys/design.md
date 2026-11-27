# Design

## D1. Где искать
`moov` → `meta` → `keys`/`ilst`. `meta` может быть full box с 4-байтовым
version/flags; пробуем payload как есть, затем без первых 4 байт.

## D2. keys
`version/flags` (4), `entry_count`, затем записи:
`size` (включая 4+4+name), `namespace` (`mdta`), `name` с NUL-добивкой.
Индекс в `ilst` — 1-based.

## D3. ilst/data
Элемент `ilst` с типом = индекс; внутри `data`:
`type` (1 = UTF-8), `locale`, value. Читаем только type 1.

## D4. Имена
`com.apple.quicktime.creationdate` → `Keys:CreationDate`;
`make/model/software` → одноимённые `Keys:*`; остальные ключи пока
игнорируются.

## D5. Дата
`YYYY-MM-DDTHH:MM:SS±HHMM` → `YYYY:MM:DD HH:MM:SS±HH:MM` (как ExifTool).
Дробные секунды отбрасываются; значение без T остаётся как есть.

## Risks

- Другие Apple-ключи (GPS, content identifier) не покрыты; добавляются
  отдельным change.
- `keys`/`ilst` могут отсутствовать в обычных MP4 — парсер просто ничего не
  добавляет.
