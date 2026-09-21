#!/usr/bin/env bash
# P0.5 size measurement (plans/06-size-and-perf.md §3 method).
# Usage: scripts/measure.sh [variant ...]   (default: all)
# Per variant: cargo build (wasm-release profile) → wasm-bindgen --target web →
# wasm-opt -Oz → gzip -9. Prints raw/gz of the optimised wasm and of the JS glue.
set -euo pipefail
cd "$(dirname "$0")/.."
VARIANTS=("$@")
[ ${#VARIANTS[@]} -eq 0 ] && VARIANTS=(base core core-f64 loc loc-cu icu-decimal-blob intl-num)
PROFILE=${PROFILE:-wasm-release}
T=target/wasm32-unknown-unknown/$PROFILE
mkdir -p out
for v in "${VARIANTS[@]}"; do
  crate=w-$v; lib=w_${v//-/_}
  CARGO_BUILD_JOBS=3 cargo build -q -p "$crate" --target wasm32-unknown-unknown --profile "$PROFILE"
  rm -rf "out/$v-$PROFILE"
  wasm-bindgen --target web --out-dir "out/$v-$PROFILE" "$T/$lib.wasm"
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "out/$v-$PROFILE/${lib}_bg.wasm" -o "out/$v-$PROFILE/opt.wasm"
  raw=$(stat -c %s "out/$v-$PROFILE/opt.wasm")
  gz=$(gzip -9 -c "out/$v-$PROFILE/opt.wasm" | wc -c)
  jsraw=$(stat -c %s "out/$v-$PROFILE/$lib.js")
  jsgz=$(gzip -9 -c "out/$v-$PROFILE/$lib.js" | wc -c)
  printf '%-20s wasm raw %8d gz %8d | js raw %6d gz %5d\n' "$v" "$raw" "$gz" "$jsraw" "$jsgz"
done
