# iptc-digest Specification

## Purpose

Синхронизация Photoshop IPTCDigest после записи IPTC.

## Requirements

### Requirement: Refresh digest
Система SHALL пересчитывать MD5 IPTC IIM и обновлять ресурс `0x0425` после
изменения IPTC.

#### Scenario: Дата в IPTC
- **WHEN** запись меняет DateCreated/TimeCreated
- **THEN** `Photoshop:IPTCDigest` равен `CurrentIPTCDigest`

### Requirement: Create digest
Если ресурса `0x0425` не было, система SHALL добавить его.

#### Scenario: Новый IPTC
- **WHEN** IPTC создаётся с нуля
- **THEN** digest присутствует и соответствует IIM
