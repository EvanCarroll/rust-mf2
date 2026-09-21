#!/usr/bin/env bash
# Builds the browser harness twice (UTF-8 validated once at load / per access)
# with the 06 §3 profile, then wasm-bindgen --target web and wasm-opt -Oz.
set -euo pipefail
cd "$(dirname "$0")/.."
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
for v in eager per-access; do
  feat=(); [ "$v" = per-access ] && feat=(--features utf8-per-access)
  CARGO_BUILD_JOBS=3 cargo build -q -p p08-web --target wasm32-unknown-unknown --profile wasm-release "${feat[@]}"
  rm -rf "out/pkg-$v"
  wasm-bindgen --target web --no-typescript --out-dir "out/pkg-$v" target/wasm32-unknown-unknown/wasm-release/p08_web.wasm
  wasm-opt -Oz "${FEATURES[@]}" "out/pkg-$v/p08_web_bg.wasm" -o "out/pkg-$v/p08_web_bg.wasm"
  printf '%s: %s B wasm, %s B gz\n' "$v" "$(stat -c %s "out/pkg-$v/p08_web_bg.wasm")" "$(gzip -9 -n -c "out/pkg-$v/p08_web_bg.wasm" | wc -c)"
done
