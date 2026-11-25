# Tasks

## 1. Базовая линия

- [x] 1.1 Зафиксировать текущие замеры `tests/bench.py` (старт, одиночный JPEG, батч) в `BENCHMARKS.md` как «до»
- [x] 1.2 Добавить тест/утилиту, считающую число разобранных групп, чтобы проверять ленивость

## 2. Ленивые таблицы

- [x] 2.1 Построить индекс RAW по group 1 и table без разбора `TagDef`; проверка — unit-тест на `lines_by_group`/`table_groups`
- [x] 2.2 Разбирать группу по требованию с кэшем и `Box::leak`; проверка — тест «после `find_name("EXIF","Make")` загружена только группа IFD0»
- [x] 2.3 Сохранить семантику неоднозначных таблиц (`Exif::Main/513` → `ThumbnailOffset`, `271` → `Make`); проверка — unit-тесты
- [x] 2.4 Хранить печатные конверсии срезом и разбирать по требованию; проверка — тест `Orientation=6` → `Rotate 90 CW`
- [x] 2.5 Реализовать `has_name`/`has_group_name` для CLI без загрузки таблиц; проверка — тест выбора `-Make` и неизвестного тега

## 3. Проверка и замеры

- [x] 3.1 `cargo test --workspace`, `clippy -D warnings`, `fmt --check` — зелёные
- [x] 3.2 `cargo xtask compare` — 0 расхождений на корпусе
- [x] 3.3 Повторить `tests/bench.py`, записать «после» в `BENCHMARKS.md` и `docs/tag-tables.md`; цель — одиночный файл <10 мс
- [x] 3.4 Обновить `docs/licensing.md`/`NOTICE`, если меняется состав данных (не требуется при неизменном `tags.tsv`)

## Workflow follow-up

- Archive the change after review.
- Verify `openspec list` is empty and the specs still validate.
