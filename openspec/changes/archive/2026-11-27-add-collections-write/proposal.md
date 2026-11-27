# Proposal

## Why

`Collections` — последний phtools-тег, не покрытый etng. Это XMP-структура
`mwg-coll:Collections` (CollectionName + CollectionURI), а не scalar.

## What Changes

- Новый writable-тег `Collections=Name|URI`, повторяемый.
- XMP `mwg-coll:Collections` как `rdf:Bag` из `rdf:li rdf:parseType="Resource"`.
- При отсутствии XMP создаётся минимальный пакет.
- Существующее значение Collections заменяется списком.

## Capabilities

### New Capabilities

- `collections-write`: XMP MWG Collections.

## Impact

- `date_sidecars.rs`, `write.rs`, CLI/help, тест.
