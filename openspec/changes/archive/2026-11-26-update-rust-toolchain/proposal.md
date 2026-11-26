# Proposal

## Why

`rustup update` установил stable 1.100.0, но проект был закреплён на 1.92.0.
Новый компилятор выявил clippy-замечания.

## What Changes

- `rust-toolchain.toml` → 1.100.0.
- CI toolchain → 1.100.0.
- MSRV/README → Rust 1.100+.
- Исправления `chunks_exact`/`as_chunks` и collapsible if/match.
- Удалены неиспользуемые зависимости `img-parts`, `rayon` из core.

## Impact

- Сборка требует Rust 1.100; поведение CLI не меняется.
