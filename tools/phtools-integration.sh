#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PHTOOLS_DIR="${PHTOOLS_DIR:-$HOME/work/phtools}"
FEATURES=("$@")
if [ ${#FEATURES[@]} -eq 0 ]; then
    FEATURES=(features/phgettags.feature features/phfixdto.feature)
fi

if ! command -v ruby >/dev/null 2>&1; then
    echo "ruby is required for phtools integration" >&2
    echo "install it first (e.g. sudo pacman -S ruby) and run bundle install in $PHTOOLS_DIR" >&2
    exit 1
fi
if [ ! -d "$PHTOOLS_DIR" ]; then
    echo "phtools directory not found: $PHTOOLS_DIR" >&2
    exit 1
fi

cargo build -q --manifest-path "$ROOT/Cargo.toml" -p etng-cli

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$ROOT/tools/exiftool-shim/exiftool" "$WORK/exiftool"
chmod +x "$WORK/exiftool"

export PATH="$WORK:$PHTOOLS_DIR/exe:$PATH"
export ETNG_BIN="$ROOT/target/debug/etng"
export ETNG_SHIM_LOG="$WORK/exiftool-shim.log"
export ETNG_SHIM_FALLBACK="${ETNG_SHIM_FALLBACK:-0}"

echo "etng:   $ETNG_BIN"
echo "shim:   $WORK/exiftool"
echo "phtools: $PHTOOLS_DIR"
echo "log:    $ETNG_SHIM_LOG"

cd "$PHTOOLS_DIR"
bundle exec cucumber "${FEATURES[@]}"
