# Design

## Context

CR3 — ISO-BMFF с Canon-расширениями. Проверено на `canon-r6.cr3` (336 КБ, exiftool 13.55):

- `ftyp`: brand `crx `, compatible `crx `, `isom`;
- `moov/uuid` (Canon) содержит `CMT1` 520, `CMT2` 1544, `CMT3` 13832, `CMT4` 2056, `THMB` 12864;
- `CMT1..CMT4` — полноценные TIFF-структуры: заголовок `II*\0`, IFD и значения со смещениями относительно начала бокса;
- `THMB`: `u16 width`, `u16 height`, `u32 jpeg_length`, 4 байта флагов, затем JPEG;
- `PRVW` лежит в отдельном `uuid`: 16-байтовый заголовок, затем JPEG (237 320 байт);
- `mvhd`/`tkhd`/`mdhd` используют эпоху 1904 года; `stsd` — видео-sample-entry с `CRAW`, размерами, 72 dpi, depth 24;
- `mdat` в образце усечён: заголовок объявляет 30 903 382 байта, доступно меньше;
- порядок групп в выводе: QuickTime → Canon → IFD0 → ExifIFD → CanonCustom → GPS → Track1..4 → XMP → Composite.

## Goals / Non-Goals

**Goals:**

- Нативное чтение CR3 без fallback, `-b` для `ThumbnailImage`/`PreviewImage`.
- Точное совпадение с exiftool на корпусе для реализованных групп.
- Устойчивость к усечённым/битым боксам без паник.

**Non-Goals:**

- Полная поддержка всех CanonCustom-функций и массивов JSON.
- Видеоформаты (MP4/MOV) как отдельные типы; только QuickTime-метаданные внутри CR3.
- Запись CR3.

## Decisions

### D1. Отдельный BMFF-walker в `cr3.rs`

Верхнеуровневые боксы читаются как обычно; Canon-боксы ищутся сканированием по сигнатуре kind с проверкой размера перед ним (они лежат внутри `uuid` без стандартной вложенности). Все срезы bounds-checked.

### D2. CMT1/CMT2 — через существующий EXIF-парсер

`CMT1` разбирается как IFD0. `CMT2` разбирается тем же путём, затем группа `IFD0` переименовывается в `ExifIFD`; таблица `Exif::Main` уже содержит нужные имена.

### D3. CMT4 — отдельный GPS-проход

GPS-теги имеют собственную таблицу `GPS::Main`; для них пишется компактный IFD-обход с преобразованием значений и координатными print-конверсиями.

### D4. CMT3 — Canon MakerNotes

`makernotes::parse` получает `Some("Canon")` и payload CMT3. В `candidate_layouts` добавляется вариант «TIFF-заголовок + IFD по `first_ifd_offset`», иначе текущие раскладки не находят IFD.

### D5. THMB/PRVW

`THMB`: длина JPEG из `u32` по смещению 4, данные с 12. `PRVW`: данные с 16 до конца бокса. Оба тега вставляются в порядке exiftool: Canon-миниатюра после CMT3, превью — в QuickTime-секцию.

### D6. QuickTime/Track

`mvhd`, `tkhd`, `mdhd`, `hdlr`, `stsd`, `stts`, `stsz`, `co64` парсятся в `Track1..Track4` по порядку. Даты конвертируются из эпохи 1904 (`-2082844800` секунд) в локальное время; длительности печатаются как `1.00 s`, проценты — `0.00%`, язык — три буквы из packed-кода.

### D7. Fallback (временно)

CR3 распознаётся нативно и `-b` никогда не делегирует. Но вложенные Canon-таблицы (CameraSettings/ShotInfo/FileInfo/CustomFunctions и ValueConv-формулы) пока не покрыты, поэтому в `auto`-режиме JSON/CSV заменяются выводом exiftool: иначе пользователь потерял бы ~90 тегов по сравнению с текущим fallback. `--fallback=never` показывает нативный набор; после `add-canon-nested-makernotes` обогащение убирается.

## Risks / Trade-offs

- **[Неполный CanonCustom]** → часть производных вендорских тегов не выводится; фиксируется в `docs/divergences.md`.
- **[Массивы]** → `CompatibleBrands` в модели строка, а не JSON-массив; документируется.
- **[Усечённый mdat]** → размеры берутся из заголовка, байты не читаются; предупреждение как у exiftool.
- **[Порядок тегов]** → строим в порядке exiftool; расхождения ловит `xtask compare`.

## Migration Plan

Аддитивно; CR2/TIFF не затрагиваются. Откат — revert коммита.

## Open Questions

Нет.
