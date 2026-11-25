# cr3-read Specification

## Purpose

Нативное чтение Canon CR3 (ISO-BMFF): метаданные EXIF/Canon/GPS, QuickTime/Track, встроенная миниатюра и превью — без fallback на системный exiftool.

## Requirements

### Requirement: CR3 detection
Система SHALL определять CR3 по `ftyp` brand `crx ` и MUST выдавать `FileType=CR3`, `FileTypeExtension=cr3`, `MIMEType=image/x-canon-cr3`.

#### Scenario: Файл без расширения
- **WHEN** подан CR3-файл с именем `photo.bin`
- **THEN** формат определяется как CR3, а не Unknown

### Requirement: Canon metadata from CMT boxes
Система SHALL извлекать IFD0, ExifIFD, MakerNotes Canon и GPS из боксов `CMT1`–`CMT4` и MUST сохранять группы `IFD0`, `ExifIFD`, `Canon`, `GPS`.

#### Scenario: CR3 Canon
- **WHEN** файл содержит CMT1–CMT4
- **THEN** в выводе присутствуют `IFD0:Make`, `ExifIFD:ExposureTime`, `Canon:Quality`, `GPS:GPSLatitude`

### Requirement: Embedded thumbnail and preview
Система SHALL извлекать JPEG-миниатюру из `THMB` как `Canon:ThumbnailImage` и превью из `PRVW` как `QuickTime:PreviewImage`, MUST сохранять их длину и MUST отдавать байты при `-b`.

#### Scenario: Миниатюра CR3
- **WHEN** выполнено `etng -b -ThumbnailImage photo.cr3`
- **THEN** возвращается 12840 байт, начинающихся с `FFD8`

#### Scenario: Превью CR3
- **WHEN** выполнено `etng -b -PreviewImage photo.cr3`
- **THEN** возвращается 237320 байт, начинающихся с `FFD8`

### Requirement: QuickTime and track metadata
Система SHALL извлекать `ftyp`/`mvhd`/`mdat` и метаданные треков (`tkhd`, `mdhd`, `hdlr`, `stsd`, `stts`, `stsz`, `co64`) в группы `QuickTime` и `TrackN`, MUST корректно переводить даты из эпохи 1904 года и длительности в секунды.

#### Scenario: Заголовок фильма
- **WHEN** файл содержит `mvhd`
- **THEN** вывод содержит `QuickTime:CreateDate`, `QuickTime:TimeScale`, `QuickTime:Duration`, `QuickTime:NextTrackID`

#### Scenario: Видеотрек
- **WHEN** файл содержит видео-трек с `stsd`
- **THEN** вывод содержит `Track1:CompressorID`, `Track1:ImageWidth`, `Track1:VideoFrameRate`

### Requirement: Truncated CR3 tolerance
Система SHALL читать доступные боксы и MUST NOT паниковать или зависать при усечённом `mdat` и повреждённых боксах.

#### Scenario: Усечённый mdat
- **WHEN** заголовок `mdat` объявляет больше байт, чем есть в файле
- **THEN** метаданные читаются, размеры берутся из заголовка, процесс завершается без паники
