# Proposal

## Why

etng обновляет UTC-даты в `mvhd`, но не Apple-ключ
`com.apple.quicktime.creationdate`. После `phfixdto` на iPhone MOV локальная
дата остаётся старой.

## What Changes

- При записи `CreateDate`/`DateTimeOriginal`/`AllDates` в MOV обновлять
  существующий `Keys:CreationDate` in-place.
- Сохранять timezone offset старого значения.
- Если Apple-ключа нет — не создавать его (пока), `mvhd` обновляется как
  раньше.

## Capabilities

### New Capabilities

- `mov-apple-date-write`: запись Apple `CreationDate`.

## Impact

- `crates/etng-core/src/write.rs`, CLI-тесты.
