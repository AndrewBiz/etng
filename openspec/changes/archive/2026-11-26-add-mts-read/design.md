# Design

## Context

M2TS — 192-байтные пакеты: 4 байта arrival timestamp + 188 байт MPEG-TS.
Внутри video PID — H.264 Annex B, где SEI type 5 содержит MDPM с UUID
`17ee8c60f84d11d98cd60800200c9a66`.

В сыром файле `MDPM` не находится: он разорван заголовками TS/PES и
emulation-prevention байтами. Поэтому нужен demux, а не поиск по файлу.

## Decisions

### D1. Сбор ES по PID
Каждый пакет: снять timestamp, разобрать adaptation field, при PUSI срезать
PES-заголовок (`9 + header_len`), остальное добавить в буфер PID. Затем ищем
сигнатуру в буфере.

### D2. Удаление emulation prevention
Перед поиском MDPM убираем `00 00 03` → `00 00`; без этого значения GPS
сдвигаются на байт.

### D3. MDPM
20-байтная сигнатура, count, записи `tag u8 + value[4]`. 0x18 комбинируется с
0x19 (8 байт); время BCD: `02 20 14 01 03 13 26 26` → `2014:01:03 13:26:26+01:00`.
GPS-рационалы — пары u16, три записи подряд.

### D4. Scope
Читаем только дату и GPS; длительность/кодеки/треки M2TS не входят.

## Risks / Trade-offs

- **[Разные packet size]** → поддержаны 192 (M2TS) и 188 (TS).
- **[Несколько video PID]** → сигнатура ищется в каждом собранном потоке.
- **[M2TS:Duration]** → пока не выводится.

## Migration Plan

Аддитивно; откат — revert.

## Open Questions

Нет.
