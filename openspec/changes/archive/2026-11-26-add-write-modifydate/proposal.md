# Proposal

## Why

`AllDates` и `-ModifyDate=` падали, если в IFD0 отсутствовал `ModifyDate`
(например, `synthetic/base.jpg`). Это последняя из трёх дат, которую нужно
вставлять для сценариев phtools `phfixdto`.

## What Changes

- Вставка отсутствующего `ModifyDate` в IFD0.
- Append-only IFD0: новый блок в конце TIFF, первая IFD в TIFF header
  переводится на него; значения и IFD1 не двигаются.
- Для JPEG пересчитывается APP1, для TIFF — header offset.
- `AllDates` обновляет/вставляет все три даты.

## Capabilities

### New Capabilities

- `modifydate-insertion`: вставка отсутствующего IFD0 ModifyDate.

## Impact

- `crates/etng-core/src/write.rs`, CLI-тесты.
