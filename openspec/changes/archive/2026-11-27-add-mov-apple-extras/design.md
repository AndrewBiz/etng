# Design

## D1. GPS
`ISO6709` вида `+55.0132+037.4721+184.497/` парсится на два координатных
числа и высоту. Координаты печатаются как ExifTool:
`55 deg 0' 47.52" N`; высота — `184.497 m Above Sea Level` (отрицательная —
`Below Sea Level`).

## D2. ContentIdentifier
Строка из `com.apple.quicktime.content.identifier` выводится как
`Keys:ContentIdentifier` без преобразований.

## D3. ©day
В `udta` → `UserData:ContentCreateDate`; в `udta/meta/ilst` → `ItemList`-
группа. Значение нормализуется тем же `apple_date`, что и `CreationDate`.

## D4. Границы
Точность GPS (`location.accuracy.horizontal`) и timed metadata не входят в
этот change. `©day` ищется только в стандартных контейнерах.
