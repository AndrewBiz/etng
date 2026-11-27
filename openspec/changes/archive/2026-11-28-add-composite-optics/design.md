# Design

## D1. ScaleFactor35efl
`FocalLengthIn35mmFormat / FocalLength`; если нет — диагональ сенсора
(`FocalPlaneDiagonal`, `SensorSize`, либо `FocalPlaneX/YResolution` +
`ExifImageWidth/Height`).

## D2. Формулы
- CoC = sqrt(24²+36²)/(scale*1440), print `%.3f mm`.
- FOV = 2*atan(36/(2*focal*scale)), print `%.1f deg`.
- Hyperfocal = focal²/(FNumber*CoC*1000), print `%.2f m`.
- GPSAltitude: значение float, print с усечением до 0.1 (как ExifTool).

## D3. SubSec
Если есть базовая дата и `SubSecTime*`, склеить `date.subsec`; offset
добавить, если он есть и дата без зоны.

## Risks
Scale factor без focal-plane данных не вычисляется; такие файлы просто не
получают оптические Composite-теги.
