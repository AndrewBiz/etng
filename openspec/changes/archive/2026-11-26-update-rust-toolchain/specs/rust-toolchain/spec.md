# rust-toolchain Specification

## Purpose

Сборка на актуальном stable Rust.

## ADDED Requirements

### Requirement: Toolchain pin
Проект SHALL собираться на Rust 1.100.0 через `rust-toolchain.toml`.

#### Scenario: Версия в проекте
- **WHEN** выполняется `cargo build` в репозитории
- **THEN** используется rustc 1.100.0

### Requirement: CI parity
CI MUST использовать тот же toolchain, что и локальная сборка.

#### Scenario: GitHub Actions
- **WHEN** запускается workflow
- **THEN** fmt/clippy/test/compare идут на 1.100.0
