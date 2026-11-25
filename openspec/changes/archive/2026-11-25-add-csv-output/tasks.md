# Tasks

## 1. Опции и фильтрация

- [x] 1.1 Добавить `csv` и `exclude` в `ReadOptions`, метод `wants`, применить в JSON/тексте/raw
- [x] 1.2 Разобрать `-csv`/`--csv` и `-x TAG`/`--exclude` (повторяемые), обновить `--help`
- [x] 1.3 Канонические имена тегов для заголовков CSV (поиск по таблицам)

## 2. CSV-вывод

- [x] 2.1 CSV-writer: заголовок, строки, экранирование запятых/кавычек/переводов строк
- [x] 2.2 Значения: print-conversion, `-n`, `-b` (base64), заглушка бинаря
- [x] 2.3 Колонки: `SourceFile` + выбор в порядке аргументов; без выбора — union алфавитно
- [x] 2.4 Двухпроходный сбор колонок без выбора; один проход при выборе
- [x] 2.5 Приоритет `-csv` над `-j`; диагностика в stderr, коды возврата прежние

## 3. Проверка

- [x] 3.1 CLI-тесты: порядок/регистр колонок, union, пустые ячейки, кавычки, `-G1`, `-n`, `-csv -b`
- [x] 3.2 CLI-тесты `-x`: убирает поле из JSON, колонку из CSV, сильнее `-TAG`; группа `IFD0:Make`
- [x] 3.3 Сравнение CSV с exiftool на корпусе (колонки и значения вне известных расхождений)
- [x] 3.4 `cargo test --workspace`, `clippy -D warnings`, `fmt --check` — зелёные
- [x] 3.5 Обновить `README.md` и `docs/divergences.md`

## Workflow follow-up

- Archive the change after review.
- Verify `openspec list` is empty and specs validate with `--strict`.
