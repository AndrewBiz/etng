# Tasks

## 1. Модель и извлечение

- [x] 1.1 Добавить `binary: bool` в `ReadOptions` и разбор `-b`/`--binary` в CLI
- [x] 1.2 Извлекать `IFD1:ThumbnailImage` для JPEG, TIFF, HEIC; вставлять после `ThumbnailLength`
- [x] 1.3 Заполнять `IFD0:PreviewImage` CR2 реальными байтами из файла вместо нулей
- [x] 1.4 Unit-тесты: миниатюра начинается с `FFD8` и имеет длину `ThumbnailLength`; CR2-превью не нулевое

## 2. Вывод

- [x] 2.1 Сырой writer в CLI: без разделителей, скаляры через `Value::display()`, байты как есть
- [x] 2.2 Порядок: выбор — порядок `-TAG`; без выбора — системные поля и теги в обычном порядке
- [x] 2.3 JSON-эмиттер: `Value::Bytes` → `"base64:<...>"` при `-b`; локальный base64-кодировщик
- [x] 2.4 Отключить fallback в режиме `-b`, диагностика в stderr, коды возврата прежние

## 3. Проверка

- [x] 3.1 CLI-тесты: `-b -Make` = `Canon` без newline; `-b -Model -Make` = порядок аргументов; `-b -ThumbnailImage` = 1378 байт JPEG
- [x] 3.2 JSON-тест: `-j -b -ThumbnailImage` начинается с `"base64:/9j/"`
- [x] 3.3 `cargo test --workspace`, `clippy -D warnings`, `fmt --check` — зелёные
- [x] 3.4 `cargo xtask compare` — 0 расхождений; добавить сравнение байтов миниатюры/превью с exiftool
- [x] 3.5 Обновить `README.md`, `docs/divergences.md`, спеку `metadata-read`

## Workflow follow-up

- Archive the change after review.
- Verify `openspec list` is empty and specs validate with `--strict`.
