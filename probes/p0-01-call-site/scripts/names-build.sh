#!/usr/bin/env bash
# P0.1 analysis aid: build one generated app with symbol names kept
# (strip=false) into its own target dir, wasm-bindgen --keep-debug, then
# twiggy top as CSV. Not a size measurement (names inflate the file).
# Usage: scripts/names-build.sh <app-dir> <out-dir>
set -euo pipefail
app=$(realpath "$1"); out=$2
probe=$(realpath "$(dirname "$0")/..")
export CARGO_TARGET_DIR=$probe/target/apps-names-$(basename "$(dirname "$app")")
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3} CARGO_PROFILE_WASM_RELEASE_STRIP=false
tpl=$(basename "$app"); tpl=${tpl#app-}
lib=workload_app_$(printf '%s' "$tpl" | tr -c 'a-zA-Z0-9' '_' | tr 'A-Z' 'a-z')
(cd "$app" && cargo build --quiet --lib --no-default-features --features hydrate \
    --target wasm32-unknown-unknown --profile wasm-release)
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --keep-debug --out-dir "$out" \
    "$CARGO_TARGET_DIR/wasm32-unknown-unknown/wasm-release/$lib.wasm"
twiggy top -n 1000000 -f csv "$out/${lib}_bg.wasm" > "$out/top.csv"
