# Design

## Context

Canon CMT3 — TIFF IFD, где часть тегов является контейнерами бинарных таблиц (AFInfo 0x12, AFInfo2 0x26, TimeInfo 0x35, FocalLength 0x02, AFMicroAdj 0x4013, VignettingCorr2 0x4016, LightingOpt 0x4018, AFConfig 0x4028, …). ExifTool описывает их как `ProcessBinaryData` с `FORMAT`/`FIRST_ENTRY`, поэтому индекс тега — это номер элемента, а не байтовое смещение.

Ключевые наблюдения по образцу `canon-r6.cr3`:

- `AFInfo2`: первые 8 значений — скаляры, далее четыре массива по `NumAFPoints` (ширины, высоты, X, Y), затем два битовых массива и `PrimaryAFPoint`.
- `LightingOpt`/`VignettingCorr2`/`AFConfig`/`TimeInfo`/`MultiExp`/`HDRInfo`/`Ambience`: `int32s`, первый элемент — размер, ключи 1..N — элементы массива.
- `TimeInfo`: минуты часового пояса → `±HH:MM`, город и DST берутся из таблицы.
- Canon APEX-значения: `FNumber = 2^(raw/64)`, `ExposureTime = 1/2^(raw/32)`, `ExposureCompensation = raw/32`, `MeasuredEV = raw/32 + 5`.

## Goals / Non-Goals

**Goals:**

- Нативно покрыть AF/Lighting/Time/AFConfig/Ambience/MultiExp/HDR/AFMicro и базовые таблицы.
- Убрать артефакты, которых exiftool не выводит.
- Сохранить fallback-обогащение до полного паритета Composite/CanonCustom.

**Non-Goals:**

- Точные значения CanonCustom `CustomControls` и сложные Compound-композиты.
- Полный `JpgFromRaw` для усечённого `mdat`.

## Decisions

### D1. Последовательные смещения вместо `id * width`
Для AFInfo2 массивы идут подряд: `8..8+N`, `8+N..8+2N` и т.д. Это исправило Heights/X/Y (432/-600/364) и AFPointsInFocus.

### D2. Табличный `int32s`-парсер
Единый проход по `defs_in_table`: значение = `bytes[id*4..]`, печать — из `TagDef`, плюс специальные конверсии для `TimeZone` и `AFConfigTool`.

### D3. Отдельные ветки для строк/рациональных значений
`LensSerialNumber` (5 байт → hex), `BatteryType` (строка после 4 байт), `DustRemovalData` (bytes), `MeasuredRGGB` (4 int16 после размера), `AFMicroAdjValue` (rational64s).

### D4. Пост-обработка
`canon_fixups` приводит APEX/EV/температуру/модель/уникальный ID и обрезает строки по NUL; `HIDDEN` убирает теги, которые exiftool не эмитит (AFInfoSize, FileNumber, ShutterCount, дубли FocalPlane).

## Risks / Trade-offs

- **[Неверные CanonCustom-значения]** → таблица Functions2 имеет собственную структуру; пока выводится частично, fallback остаётся.
- **[Composite]** → производные (`DriveMode`, `Lens`, `DOF`, …) ещё не считаются нативно.
- **[Массивы]** → в модели нет JSON-массивов, AF-массивы выводятся строкой; документированное расхождение.

## Migration Plan

Аддитивно; `auto`-режим по-прежнему отдаёт полный JSON через exiftool. Откат — revert коммита.

## Open Questions

Нет.
