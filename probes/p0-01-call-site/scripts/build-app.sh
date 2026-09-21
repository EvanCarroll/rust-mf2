#!/usr/bin/env bash
# P0.1: build one generated app's client wasm exactly as plans/06 §3 says and
# append its sizes to a TSV.
#   wasm32-unknown-unknown, profile wasm-release (opt-level z, fat LTO, cgu 1,
#   panic abort, strip) -> wasm-bindgen -> wasm-opt -Oz -> gzip -9
# Usage: scripts/build-app.sh <app-dir> <results.tsv>
# Env:   CARGO_TARGET_DIR (default: <probe>/target/apps-<workload>).
set -euo pipefail
app=$(realpath "$1")
out=$2
probe=$(realpath "$(dirname "$0")/..")
# One target dir per workload: apps of different workloads share package
# names, and cargo would take one for the other (bench/workload-gen README).
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$probe/target/apps-$(basename "$(dirname "$app")")}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
tpl=$(basename "$app"); tpl=${tpl#app-}
# Package name = workload-app-<template, non-alphanumerics as '-'>.
lib=workload_app_$(printf '%s' "$tpl" | tr -c 'a-zA-Z0-9' '_' | tr 'A-Z' 'a-z')
pkg=$(dirname "$app")/pkg-$tpl
t0=$(date +%s)
(cd "$app" && cargo build --quiet --lib --no-default-features --features hydrate \
    --target wasm32-unknown-unknown --profile wasm-release)
t1=$(date +%s)
wasm=$CARGO_TARGET_DIR/wasm32-unknown-unknown/wasm-release/$lib.wasm
rm -rf "$pkg"; mkdir -p "$pkg"
wasm-bindgen --target web --no-typescript --out-dir "$pkg" "$wasm"
bg=$pkg/${lib}_bg.wasm
# Rust 1.98 wasm32-unknown-unknown default target features:
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue "$bg" -o "$pkg/opt.wasm"
sz() { stat -c %s "$1"; }
gz() { gzip -9 -c "$1" | wc -c; }
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(basename "$(dirname "$app")")" "$tpl" \
    "$(sz "$wasm")" "$(sz "$bg")" "$(gz "$bg")" "$(sz "$pkg/opt.wasm")" "$(gz "$pkg/opt.wasm")" >> "$out"
echo "$tpl: cargo $(sz "$wasm") bindgen $(sz "$bg")/$(gz "$bg")gz opt $(sz "$pkg/opt.wasm")/$(gz "$pkg/opt.wasm")gz (build $((t1-t0)) s)"
