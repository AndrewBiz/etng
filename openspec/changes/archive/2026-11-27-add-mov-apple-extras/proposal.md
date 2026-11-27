# Proposal

## Why

После `Keys:CreationDate` в iPhone MOV остаются полезные Apple-ключи:
GPS, content identifier и `©day`. ExifTool читает их, etng пока нет.

## What Changes

- `com.apple.quicktime.location.ISO6709` → `Keys:GPSCoordinates`
  (deg/min/sec + altitude, как ExifTool).
- `com.apple.quicktime.content.identifier` → `Keys:ContentIdentifier`.
- `©day` → `UserData:ContentCreateDate` / `ItemList:ContentCreateDate`.
- Unit-тест синтетического MOV; проверка на реальных iPhone MOV.

## Capabilities

### New Capabilities

- `mov-apple-extras`: GPS, identifier и `©day` из MOV.

## Impact

- `crates/etng-core/src/mov.rs`, тесты.
