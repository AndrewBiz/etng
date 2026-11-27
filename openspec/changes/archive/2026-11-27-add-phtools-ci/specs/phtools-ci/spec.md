# phtools-ci Specification

## Purpose

CI-проверка etng на сценариях phtools.

## ADDED Requirements

### Requirement: CI runs stable phtools features
CI SHALL запускать `phgettags` и `phfixdto` через etng shim.

#### Scenario: Успешный прогон
- **WHEN** CI job `phtools` выполняется
- **THEN** `tools/phtools-integration.sh` завершается с кодом 0

### Requirement: Pinned phtools revision
CI MUST использовать закреплённый commit phtools.

#### Scenario: Обновление phtools
- **WHEN** upstream phtools меняется
- **THEN** CI продолжает использовать закреплённый commit до отдельного изменения
