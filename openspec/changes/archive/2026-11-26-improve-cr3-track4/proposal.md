# Proposal

## Why

На реальных CR3 (R50) нативный разбор давал 393 из 577 тегов exiftool:
не читался `Track4` CTMD (142 тега ColorData и др.), FilterInfo, FocusBracketing,
LevelInfo и часть Composite. Это главный оставшийся разрыв CR3.

## What Changes

- Разбор CTMD-сэмпла Track4: поиск вложенных TIFF-блоков, Canon MakerNote,
  вывод тегов в группе `Track4` с дедупликацией.
- `SampleTime`, `SampleDuration`; ColorData дополнен `PerChannelBlackLevel`,
  `NormalWhiteLevel`, `SpecularWhiteLevel`, `LinearityUpperMargin`.
- FilterInfo (`0x4024`): 10 creative-фильтров.
- FocusBracketingInfo (`0x4053`) и LevelInfo (`0x4059`: Roll/Pitch/FocalLength2).
- Composite для CR3: ISO, Lens, Rotation, WB_RGGBLevels, Red/BlueBalance.
- CR3 auto по-прежнему дополняется exiftool: остаются CanonCustom и сложные
  Composite.

## Capabilities

### New Capabilities

- `cr3-track4-metadata`: CTMD/Track4, фильтры, focus bracketing, level info.

### Modified Capabilities

Нет.

## Impact

- `crates/etng-core/src/cr3.rs`, `read.rs`.
- `xtask compare` остаётся 0 (новые Composite только для CR3).
- На R50: 548/577 тегов против 393/577 до изменения.
