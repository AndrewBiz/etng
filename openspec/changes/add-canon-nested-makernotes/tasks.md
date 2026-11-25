# Tasks

## 1. Таблицы

- [x] 1.1 CameraSettings/ShotInfo/Processing/SensorInfo/FileInfo/CropInfo/AspectInfo
- [x] 1.2 AFInfo и AFInfo2 с последовательными массивами
- [x] 1.3 AFConfig/LightingOpt/VignettingCorr2/TimeInfo/MultiExp/HDRInfo/Ambience
- [x] 1.4 FocalLength, AFMicroAdj, LensSerialNumber, BatteryType, DustRemovalData, MeasuredRGGB
- [x] 1.5 CanonCustom Functions2 (частично)

## 2. Конверсии

- [x] 2.1 APEX: FNumber/MaxAperture/MinAperture, ExposureTime, ExposureCompensation
- [x] 2.2 MeasuredEV/EV2, CameraTemperature, SelfTimer, FocalUnits, FocalLength
- [x] 2.3 ImageUniqueID hex, LensModel trim, PictureStyle, AFConfigTool
- [x] 2.4 Подавление служебных тегов (AFInfoSize, FileNumber, дубли FocalPlane)

## 3. Осталось

- [ ] 3.1 Точные значения CanonCustom (`CustomControls`, диапазоны)
- [ ] 3.2 Composite-производные (`DriveMode`, `Lens`, `DOF`, `FOV`, `SubSec*`, `GPSAltitude`)
- [ ] 3.3 JpgFromRaw и Warning для усечённого mdat
- [ ] 3.4 Убрать exiftool-обогащение CR3 после 3.1–3.3

## 4. Проверка

- [x] 4.1 Нативный CR3 без fallback: 396 полей, Canon 185
- [x] 4.2 `cargo test`, `clippy -D warnings`, `fmt --check`
- [x] 4.3 `cargo xtask compare` — 0 расхождений (auto-режим)

## Workflow follow-up

- Archive after tasks 3.1–3.4 are closed.
