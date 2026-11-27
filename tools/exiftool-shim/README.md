# exiftool shim for phtools

This directory contains an `exiftool`-compatible launcher used to run
[phtools](https://github.com/AndrewBiz/phtools) against etng.

## What it does

- **Read** calls are passed to `etng`.
- **Write** calls (`exiftool -@ script`) are translated to etng assignments:
  - `-MWG:DateTimeOriginal=...` → `-DateTimeOriginal=...`
  - `-MWG:City=...` → `-City=...`
  - `-MWG:Keywords+=...` → `-Keywords=...`
  - `-XMP-mwg-coll:Collections+={CollectionName=..., CollectionURI=...}` →
    `-Collections=Name|URI`
  - `-FileModifyDate<DateTimeOriginal` → reads DTO, then writes
    `-FileModifyDate=...`
  - `-overwrite_original`, `-v0`, `-ignoreMinorErrors` are ignored.
- Every command is logged to `ETNG_SHIM_LOG`.

## Run the integration

Requirements:

- Ruby + phtools dependencies (`bundle install` in phtools);
- built etng (`cargo build -p etng-cli`).

```bash
cd ~/work/etng
./tools/phtools-integration.sh
# or specific features
./tools/phtools-integration.sh features/phgettags.feature
```

The script puts the shim first in `PATH`, sets `ETNG_BIN` to the local etng and
runs phtools' cucumber features. Set `PHTOOLS_DIR` if phtools is not at
`~/work/phtools`.

## Strict mode

By default a failed etng read **does not** silently fall back to exiftool:
the integration should expose gaps. Set:

```bash
ETNG_SHIM_FALLBACK=1 ./tools/phtools-integration.sh
```

to fall back for reads, or `ETNG_SHIM_WRITE_FALLBACK=1` to fall back for a
whole write script. The fallback events are recorded in the shim log.

## Smoke tests

The translation layer has standalone tests that do not need Ruby or exiftool:

```bash
python3 tools/exiftool-shim/test_shim.py
```

## Limits

- This is a test adapter, not a full exiftool reimplementation.
- Unsupported read options cause etng to fail under strict mode; that is
  intentional, so the phtools suite shows exactly which option is missing.
- The shim currently translates the tag names used by phtools; arbitrary
  exiftool scripts are not supported.
