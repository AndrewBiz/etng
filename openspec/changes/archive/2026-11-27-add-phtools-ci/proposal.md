# Proposal

## Why

Интеграция etng с phtools проверялась только локально. Нужен CI-гейт,
который ловит регрессии чтения/записи на стабильных сценариях phtools.

## What Changes

- Новая job `phtools` в CI:
  - Ruby 3.4;
  - checkout phtools на зафиксированный commit;
  - добавление `erb`/`mutex_m` (Ruby 3.4);
  - `bundle install`;
  - `tools/phtools-integration.sh` (phgettags + phfixdto).
- phtools-версия закреплена commit'ом, чтобы CI не ломался от upstream.

## Capabilities

### New Capabilities

- `phtools-ci`: автоматическая проверка интеграции.

## Impact

- `.github/workflows/ci.yml`, README.
