# Proposal

## Why

`Keywords` и `Creator` — списки MWG, но writer заменял их одним значением.
phtools добавляет несколько ключевых слов/авторов.

## What Changes

- Повторяемые `-Keywords=` и `-Creator=` собираются в список.
- XMP `dc:subject` пишется как `rdf:Bag`, `dc:creator` — как `rdf:Seq`.
- IPTC 2:25/2:80 — по записи на значение.
- EXIF Artist — значения через `; `.
- Повторные scalar-теги используют последнее значение.

## Capabilities

### New Capabilities

- `mwg-multivalue`: списки Keywords/Creator.

## Impact

- `date_sidecars.rs`, `write.rs`, CLI-тест.
