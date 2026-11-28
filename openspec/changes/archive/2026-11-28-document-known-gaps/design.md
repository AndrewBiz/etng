# Design

## D1. Один источник истины
`docs/divergences.md` — список осознанных ограничений; README ссылается на
него из раздела «Ограничения».

## D2. Что считать ограничением
Только то, что проверено и принято:
- отсутствующая запись MTS;
- неполные вендорные MakerNotes и Composite;
- ограничения phtools (CR2, IPTC, MOV UTC);
- output-сериализаторы и `-listw`.

## D3. Что не считать ограничением
Sony PreviewImage, PNG/HEIC/CR3/MOV dates, оптические Composite и rename/move
аудит уже реализованы и проверены; они перечислены как implemented, а не gaps.
