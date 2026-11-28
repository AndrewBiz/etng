# Documented divergences and conscious limitations

Status: **v0.28.0**, 2026-11-28.

`cargo xtask compare` reports **0 divergences** outside the groups and keys
listed here. The entries below are deliberate scope decisions and known
limitations, not silent bugs. They were collected from the parity corpus,
the phtools integration suite and a full-collection audit on 19 files.

## Read mode

### Vendor and container groups

| group | status | limitation |
| --- | --- | --- |
| `Apple:` | partial | Flat IFD subset only. `RunTime*`, `AccelerationVector`, `AEMatrix` and plist-backed derived tags are skipped. |
| `QuickTime:` | partial | CR3 movie/track headers and HEIC `ftyp`/`hvcC`/`colr`/`ispe`/`irot`/`pixi`/`auxC`/`mdat` are native. Full track tables, codecs and auxiliary images are not. `CompatibleBrands` is a string, not a JSON array. |
| `Meta:` | absent | HEIF meta-box internals (`MetaImageSize`, `PrimaryItemReference`). |
| `IFD1:` | partial | Thumbnail tags and real `ThumbnailImage` bytes are emitted. `ThumbnailOffset` may differ because exiftool reports an adjusted base offset. |
| `ICC_Profile:` / `ICC-header:` / `ICC-meas:` / `ICC-view:` | supported/partial | Common tags from JPEG APP2, TIFF `0x8773` and HEIC `colr/prof`. Uncommon tag types are skipped. |
| `Vivo:` | absent | Vivo JSON metadata blob is not decoded. |

### Remaining Composite / derived gaps

Implemented in v0.28.0 and no longer limitations: `Composite:CircleOfConfusion`,
`FOV`, `HyperfocalDistance`, `ScaleFactor35efl`, `GPSAltitude`,
`SubSecDateTimeOriginal`/`SubSecCreateDate`/`SubSecModifyDate` and basic
`RedBalance`/`BlueBalance` when `WB_RGGBLevels` exists.

Still missing or approximate:

- `Composite:DOF` — needs a reliable focus distance; not derived yet.
- `Composite:Lens`, `Composite:Lens35efl` — only partly derived from
  `MinFocalLength`/`MaxFocalLength`.
- `Composite:LensID` — exact lens-name mapping is vendor-specific. The native
  value can differ from exiftool for Canon RF/EF and Sony lenses.
- `Composite:DriveMode`, `Composite:ShootingMode` — vendor-specific mappings.
- `Composite:SubSecModifyDate` — not emitted for a few files where only some
  SubSec fields exist.
- `Composite:RunTimeSincePowerUp` and Apple `RunTimeEpoch/Flags/Scale/Value` —
  Apple plist/runtime fields are not decoded.
- Sony `RedBalance`/`BlueBalance` when only Sony-private WB levels are present.
- `ExifIFD:LensInfo` formatting and several Canon `Composite`/`LensInfo`
  value conventions.
- XMP edge tags such as `XMP-photoshop:History`, `XMP-xmpMM:DocumentID`,
  `XMP-exif:Flash*`, `XMP-crs:ToneCurve`, `XMP-tiff:BitsPerSample`,
  `XMP-tiff:Orientation`.

On the 19-file audit corpus this leaves about **1490 missing tags, 108 extra
tags and 333 value differences**; nearly all are vendor MakerNote or derived
tags. These numbers are a snapshot, not a contract.

### MakerNotes and preview offsets

- MakerNote support is a subset: Canon, Nikon, Sony and a flat Apple IFD.
  Unknown vendors produce a warning and no vendor tags.
- Canon nested tables cover CameraSettings, ShotInfo, Processing, SensorInfo,
  FocalLength, AFMicroAdj, AFInfo/2, AFConfig, LightingOpt, VignettingCorr2,
  TimeInfo, MultiExp, HDRInfo and Ambience. CanonCustom, some ColorData in CR3
  Track4 and complex Canon composites remain approximate.
- Sony ARW: first SubIFD and `PreviewImage` are native; the encrypted
  SR2SubIFD block and some Sony binary tables are not decoded.
- **Sony JPEG `PreviewImage` is the only known absolute MakerNote preview
  offset that needs shifting when APP1 grows.** It is MakerNote `0x2001` and is
  adjusted by the file-size delta for date and text writes.
- Canon JPEG MPF offsets are relative to the MPF header, and CR2/ARW preview
  offsets are TIFF-relative; append-at-end or in-place writes leave them valid.
  A stress audit found no new preview warnings for Canon JPEG, iPhone JPEG,
  CR2, ARW, HEIC or CR3 after sidecar growth.
- No universal MakerNote offset repair is enabled: Nikon, Panasonic, Olympus,
  Fujifilm and Samsung samples are needed before adding one. CR3 may retain the
  pre-existing `Odd offset for ExifIFD tag 0x9011 OffsetTimeOriginal` warning.

## Write mode

| format | dates | MWG text / GPS | notes |
| --- | --- | --- | --- |
| JPEG | yes | yes | EXIF dates; XMP/IPTC sidecars; GPS IFD; Sony preview offset repaired; IPTC digest refreshed. |
| TIFF / DNG / NEF / CR2 / ARW | yes | no | Dates updated in place; missing dates appended. No JPEG sidecars. |
| PNG | yes | no | `eXIf` chunk updated or created before `IDAT`; CRC rebuilt. XMP dates in `iTXt`/`tEXt` are not updated. |
| HEIC / CR3 | yes | no | Existing dates updated; missing dates appended (EXIF item / CMT box). Full CR3 native read still falls back in `auto`. |
| MOV | yes | no | `mvhd` dates and Apple `Keys:CreationDate` in-place where present. Tracks/codecs are not rewritten. |
| MTS / M2TS | **no** | no | Reading is native, but H.264 SEI MDPM date writing is not implemented. `phfixdto` therefore reports `date writing is not supported for MTS`; phtools itself may still exit 0. |

Details:

- `DateTimeOriginal`/`CreateDate` are written to ExifIFD and `ModifyDate` to
  IFD0 when the destination format has those structures.
- PNG date writing is EXIF-authoritative: XMP date properties in PNG are left
  unchanged rather than silently desynchronised.
- MWG text tags, `Collections` and GPS are JPEG-only. Text/GPS writes to
  TIFF-family, HEIC, CR3, MOV, PNG or MTS are rejected explicitly.
- `FileModifyDate` is applied after the atomic replacement; it is independent
  of container support.
- All writes are atomic (temp file + rename), preserve permissions and do not
  rewrite unknown MakerNote bytes.

## phtools integration

The `tools/exiftool-shim` makes phtools use etng for reads and `-@` writes.

- Full local suite: **64/65** scenarios with etng. The remaining `phrename`
  case uses IPTC `DateCreated` (`20020202`) and expects a `DateTime`; it fails
  with the real exiftool too, so it is a phtools limitation.
- CI runs the stable subset (`phgettags` + `phfixdto`): **12/12 scenarios,
  60/60 steps**.
- phtools does not support CR2: `phls` omits it and `phrename` says
  `unsupported type`; this is outside etng.
- `phrename` uses QuickTime `CreateDate` (UTC) for MOV rather than Apple
  `Keys:CreationDate` (local); this is phtools/MiniExiftool behaviour.
- MTS date failures are surfaced by etng, but phtools currently exits 0 for
  that command.
- The shim deliberately drops exiftool-only read options (`-c`, `-charset`,
  `-lang`, `-api`, `-d`) and falls back to a real exiftool only when
  `ETNG_SHIM_FALLBACK=1`; fallback is off by default.

## CLI and output

- `-json`, `-csv`, `-b`, `-x`, `-G1`, `-list` and `-listw` are implemented.
  `-X`, `-php`, `-html` and other serializers are not.
- `-listw` prints all known tag names, not a strictly exact writable-tag set.
- Warning text is etng's own; the comparator masks warnings.
- Tag order follows etng's deterministic parse order. It matches exiftool on the
  corpus but is not guaranteed byte-for-byte for every file.
- Print conversions come from `exiftool -listx` values plus handwritten rules;
  unsupported print conversions fall back to the raw value. `-n` always gives
  the machine value.
- `-b` never delegates to exiftool; formats that rely on fallback (notably some
  CR3 reads) produce no binary output. EXIF thumbnails and CR2/ARW previews are
  native.
- CSV columns are the union of etng's emitted tags, so partial groups can
  produce a shorter header than exiftool. `-csv` without `-TAG` reads the input
  twice; `-TAG` uses one pass.
- XMP multi-values are joined with `, ` and represented as strings, except
  booleans and numeric/rational strings.

## Safety and verification

- Malformed fixtures (`broken.jpg`, `endless_loop.jpg`, `invalid-gps`) are
  exercised for termination and partial output, not tag parity.
- A deterministic mutation harness (`tests/mutations.rs`) runs with the test
  suite; a full `cargo-fuzz` campaign is not part of the release gate.
- Before release: `cargo test --workspace`, clippy with `-D warnings`, fmt,
  `TZ=Europe/Moscow cargo run -p xtask -- compare` and
  `tools/corpus-audit.py` on copies.

## Not implemented by design

- Video/audio track and codec tables (including H.264 MTS date writing).
- XMP/ICC writing outside the JPEG MWG path.
- Vendor-specific preview-offset shifts without a representative sample.
- A second EXIF backend; etng keeps its own ISO-BMFF walker.
- Full exiftool serializer compatibility (`-X`, `-php`, `-html`).
