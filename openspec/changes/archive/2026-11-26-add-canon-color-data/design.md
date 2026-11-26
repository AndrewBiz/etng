# Design

## Context

`0x4001` — `int16u[count]` в Canon MakerNotes. Номер таблицы зависит от
количества значений и модели, но первая int16 — `ColorDataVersion`, поэтому
достаточно версии. Смещения в `Canon.pm` заданы в элементах, не в байтах.

Проверено на JPEG R50 (version 48, count 3778): смещения 0x69/0x6d, 0x6e/0x72,
0x73/0x77, 0xcd/0xd1, 0xd2/0xd6, 0xd7/0xdb, 0xdc/0xe0, 0xe1/0xe5, 0xe6/0xea,
0xeb/0xef дают значения, побайтово совпадающие с exiftool.

## Decisions

### D1. Версия вместо count
`count` неоднозначен (3778 встречается у ColorData11 и ColorData12). Версия
`0x00` однозначно выбирает таблицу.

### D2. Таблицы смещений
Константы `COLOR_DATA_10/11/12` повторяют `Canon.pm`; для quad-тегов читаются
четыре последовательных int16s, для температур — одна.

### D3. Группа
Теги выводятся как `Canon:*` (family 1), как у exiftool для JPEG. Для CR3
exiftool относит их к `Track4`; это отдельный путь и пока не покрыто.

## Risks / Trade-offs

- **[Неверная модель]** → версия неизвестна — теги не выводятся, fallback
  сохраняется.
- **[CR3 Track4]** → ColorData в CR3-треке не читается нативно; `auto`
  дополняет вывод exiftool.

## Migration Plan

Аддитивно; откат — revert.

## Open Questions

Нет.
