# Documented divergences from ExifTool 13.55

`cargo xtask compare` reports **0 divergences** outside the groups and keys
listed here. Entries are deliberate scope decisions, not silent bugs.

## Vendor / container groups not modelled

| group | status | reason |
| --- | --- | --- |
| `Apple:` | partial | Flat IFD subset only. Complex plist payloads (`AEMatrix`, `RunTime*`, `AccelerationVector`) and exiftool's derived Apple sub-tags are skipped. |
| `QuickTime:` | absent | ISO-BMFF track/property metadata (brands, HEVC configuration, auxiliary images) is out of scope for the read-only MVP. |
| `ICC_Profile:`, `ICC-header:`, `ICC-meas:`, `ICC-view:` | absent | ICC profile parsing is a separate subsystem. |
| `Meta:` | absent | HEIF meta-box internals (`MetaImageSize`, `PrimaryItemReference`). |
| `IFD1:` | partial | Thumbnail IFD tags are emitted, including real `ThumbnailImage` bytes (JPEG/TIFF/HEIC); only `ThumbnailOffset` differs because exiftool reports an adjusted base offset. |
| `Vivo:` | absent | Vivo JSON metadata blob is not decoded. |

## Known missing or changed composite/derived tags

`Composite:GPSAltitude`, `Composite:FocalLength35efl` (sensor-based 35 mm
factor), `Composite:Flash`, `Composite:LensID` for Nikon,
`Composite:CircleOfConfusion`, `Composite:FOV`, `Composite:HyperfocalDistance`,
`Composite:ScaleFactor35efl`, `Composite:SubSec*`,
`Composite:RunTimeSincePowerUp`, `File:Comment`,
`ExifIFD:MakerNoteUnknownText`, `XMP-photoshop:History`,
`XMP-xmpMM:DocumentID`, `XMP-exif:Flash*`, `XMP-crs:ToneCurve`,
`XMP-tiff:BitsPerSample`, `XMP-tiff:Orientation`.

Reasons: values require sensor geometry, vendor-specific structures, or
multi-valued JSON arrays that the current model does not represent.

## Hostile inputs skipped in the golden diff

`broken.jpg`, `endless_loop.jpg`, `invalid-gps` are deliberately malformed.
etng must terminate quickly and produce a partial report; exiftool recovers
more tags. These files are exercised for safety (no panic/OOM/hang), not for
tag parity.

## Behavioural notes

- **Warning text** differs (`ExifTool:Warning` is masked in the comparator);
  etng emits its own diagnostics.
- **Tag order** follows etng's deterministic parse order. It matches exiftool
  on the corpus but is not guaranteed byte-for-byte for every format.
- **Print conversions** come from `exiftool -listx` `<values>` plus hand-written
  rules (ExposureTime, ApertureValue/MaxApertureValue, FNumber,
  ShutterSpeedValue, FocalLength/FocalLengthIn35mmFormat, FocalPlane*Resolution,
  GPS coordinates/altitude/time, LensInfo, version words). Tags outside that
  set fall back to the raw value; `-n` always yields the machine value.
- **MakerNotes** support is a subset: Canon, Nikon, Sony and a flat Apple IFD.
  Unknown vendors produce a warning and no tags.
- **DNG/NEF/CR2 detection** uses the file extension to disambiguate TIFF-family
  containers when the signature alone is ambiguous (exiftool does the same for
  extension-based TIFF variants). CR2 preview tags (273/279) are re-interpreted
  as `PreviewImageStart`/`PreviewImageLength`, matching exiftool; `-b` returns
  the real preview bytes.
- **`-b` and fallback**: binary mode never delegates to the system exiftool, so
  formats that rely on fallback (for example CR3) produce no binary output.
  EXIF-embedded thumbnails and CR2 previews are extracted natively.
- **`-b` scalar values** are printed without print conversion (`-Orientation`
  yields `1`), and selected tags are concatenated in argument order, matching
  exiftool. JSON mode emits binary values as `"base64:<data>"`.
- **`-b` without a tag selection** follows etng's stored values. Tags kept as
  normalized strings (for example `ExifVersion` or `YCbCrSubSampling`) may
  differ from exiftool's raw byte-list form; use `-b -TAG` for extraction.
- **XMP multi-values** are joined with `, ` and represented as strings, except
  booleans (`True`/`False` → JSON `true`/`false`) and numeric/rational strings
  which become JSON numbers.
- **Fuzzing**: a deterministic mutation harness (`tests/mutations.rs`) runs
  with every test suite; a full `cargo-fuzz` campaign is not part of this
  change.

## Not implemented by design

Write mode, `-csv`/`-X`/`-php`/`-html` output, video/audio containers, XMP/ICC
writing, `nom-exif` as a second backend (own ISO-BMFF walker is used instead).
