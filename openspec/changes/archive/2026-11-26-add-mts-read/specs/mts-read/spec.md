# mts-read Specification

## Purpose

Чтение даты и GPS из MTS/M2TS (AVCHD) через H.264 SEI MDPM.

## ADDED Requirements

### Requirement: M2TS demux
Система SHALL определять 192-байтные M2TS и 188-байтные TS пакеты, снимать
timestamp/adaptation/PES-заголовки и MUST собирать elementary stream по PID.

#### Scenario: M2TS
- **WHEN** файл начинается с 4-байтного timestamp и 0x47 на смещении 4
- **THEN** FileType = `MTS`, видеопоток собирается по PID

### Requirement: MDPM date
Система SHALL удалять H.264 emulation-prevention байты и MUST разбирать
MDPM-запись 0x18+0x19 в `H264:DateTimeOriginal`.

#### Scenario: Дата AVCHD
- **WHEN** MDPM содержит `02 20 14 01 03 13 26 26`
- **THEN** `H264:DateTimeOriginal` = `2014:01:03 13:26:26+01:00`

### Requirement: MDPM GPS
Система MUST выводить `GPS:GPSVersionID`, `GPSLatitude/Ref` и
`GPSLongitude/Ref` из MDPM.

#### Scenario: Координаты
- **WHEN** MDPM содержит 51/1, 12/1, 34346/1000 и N
- **THEN** `GPS:GPSLatitude` = `51 deg 12' 34.35"`,
  `GPS:GPSLatitudeRef` = `North`
