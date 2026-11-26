# arw-subifd Specification

## Purpose

SubIFD-метаданные и preview-изображение Sony ARW.

## ADDED Requirements

### Requirement: SubIFD tags
Система SHALL читать первый SubIFD из TIFF-тега `0x014a` и MUST выводить его
известные теги в группе `SubIFD`.

#### Scenario: Размеры RAW
- **WHEN** SubIFD содержит `ImageWidth=3936` и `ImageHeight=2624`
- **THEN** вывод содержит `SubIFD:ImageWidth` и `SubIFD:ImageHeight`

### Requirement: Sony preview extraction
Для `Make=SONY` система SHALL извлекать preview по `IFD0:0x0201/0x0202` и MUST
проверять, что байты начинаются с JPEG-маркера `FFD8`.

#### Scenario: PreviewImage
- **WHEN** ARW содержит preview 1118163 байт по смещению 145058
- **THEN** `-b -PreviewImage` отдаёт эти байты, а JSON содержит
  `PreviewImageStart`, `PreviewImageLength` и placeholder `PreviewImage`

#### Scenario: Не JPEG
- **WHEN** по указанному смещению нет `FFD8`
- **THEN** preview-теги не выводятся
