# Test corpus provenance

All files are used unchanged, only for compatibility testing.

## Synthetic files (`tests/corpus/synthetic/`)

Generated for this project with Python PIL and tagged with ExifTool 13.55:

| file | origin | license |
| --- | --- | --- |
| `base.jpg` | PIL 13.0.0, 64×48 red square; EXIF/IPTC/XMP written by `exiftool -overwrite_original` | project-owned, MIT |
| `base.tif` | PIL 13.0.0, 64×48 red square | project-owned, MIT |
| `dng_sample.dng` | copy of `base.tif` with `DNGVersion=1.4` and `Make=NIKON CORPORATION` | project-owned, MIT |
| `nef_sample.nef` | copy of `base.tif` with `Make=NIKON CORPORATION`, `Model=NIKON Z 6` | project-owned, MIT |
| `truncated.jpg` | first half of `base.jpg`, byte-identical prefix | project-owned, MIT |
| `truncated.heic` | first 1024 bytes of `real/exif.heic` | derived, see below |

## Real files (`tests/corpus/real/`)

| file | origin | license |
| --- | --- | --- |
| `exif.heic` | `nom-exif` testdata: https://github.com/mindeng/nom-exif/blob/main/testdata/exif.heic | MIT OR Apache-2.0 (upstream repository) |
| `exif.jpg` | same repository, `testdata/exif.jpg` | MIT OR Apache-2.0 |
| `tif.tif` | same repository, `testdata/tif.tif` | MIT OR Apache-2.0 |
| `canon-r6.cr3` | same repository, `testdata/canon-r6.cr3` (format not supported → fallback test) | MIT OR Apache-2.0 |
| `broken.jpg`, `endless_loop.jpg`, `invalid-gps` | same repository (hostile inputs, skipped in golden diff) | MIT OR Apache-2.0 |
| `Canon_40D.jpg`, `Canon_40D_photoshop_import.jpg`, `Nikon_D70.jpg`, `Sony_HDR-HC3.jpg` | `ianare/exif-samples`: https://github.com/ianare/exif-samples (`jpg/`) | test samples published for ExifTool testing; see upstream repository |

`truncated.heic` is a derived byte prefix of `exif.heic` and inherits its
license.
