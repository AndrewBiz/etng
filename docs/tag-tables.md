# Snapshot procedures

## `exiftool -listx` tag tables

`crates/etng-tags/data/listx.xml` is a verbatim snapshot of:

```bash
exiftool -listx -f > crates/etng-tags/data/listx.xml
```

taken with the compatibility target version (see `EXIFTOOL_COMPAT_VERSION` in
`crates/etng-core/src/compat.rs`). The snapshot is committed so builds stay
offline and reproducible.

Regenerate the compact embedded table after replacing the snapshot:

```bash
cargo xtask generate-tags
```

The generated `crates/etng-tags/data/tags.tsv` has these columns:

| column | meaning |
| --- | --- |
| 1 | family 0 group (`EXIF`, `MakerNotes`, `XMP`, `IPTC`, `File`) |
| 2 | family 1 group (`IFD0`, `ExifIFD`, `GPS`, `XMP-dc`, `Canon`) |
| 3 | ExifTool table name (`Exif::Main`, `Canon::Main`) |
| 4 | table-local id (decimal for binary formats, local name for XMP) |
| 5 | ExifTool tag name |
| 6 | ExifTool type |
| 7 | writable flag (`1`/`0`) |
| 8 | print-conversion pairs, `id` + `\x1e` + label, separated by `\x1d` |

Regeneration is deterministic: running `cargo xtask generate-tags` twice in a
row leaves `tags.tsv` unchanged (`git diff --exit-code` is clean). Update the
snapshot and the table in one commit together with the golden corpus.
