# Design

## D1. Копии
Все операции — только над `tempfile.mkdtemp`; оригиналы не открываются на
запись. Для rename и fix используются отдельные копии.

## D2. Сравнение
`etng -j -G1` и `exiftool -j -G1`; игнорируются SourceFile/FileName/Directory
и volatile access-даты. Считаются missing/extra/value-diff, top-теги и
разбивка по файлам.

## D3. phtools
`phls`, `phrename -a anb`, `phfixdto` через shim; повторные прогоны и
sha256 для идемпотентности. `exiftool -validate` до и после; ошибки — fail,
предупреждения — отчёт.

## D4. Рекурсия
Shim запускает etng с `--fallback=never`: иначе auto-fallback etng вызывает
`exiftool` из PATH, то есть shim, и получается бесконечная цепочка.

## D5. Тайминги
`time.perf_counter()` вокруг копирования, etng/exiftool read, каждой
phtools-команды, validate и сравнения.
