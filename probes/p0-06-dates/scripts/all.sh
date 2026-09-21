#!/usr/bin/env bash
# Everything RESULT.md reports, in order.
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BUILD_JOBS=3 cargo test -q -p mf2dt
scripts/measure.sh base sem intl intl-inline icu-blob-fixed icu-blob-fixed-nozone icu-blob-fixed-offz icu-blob-any icu-compiled-fixed icu-compiled-any | tee out/sizes.txt
scripts/blobs.sh | tee out/blob-sizes.txt
scripts/b12.sh | tee out/b12.txt
CARGO_BUILD_JOBS=3 cargo run -q -p check --release --bin check | tee out/check.txt
for v in intl intl-inline; do
  lib=w_${v//-/_}
  wasm-bindgen --target nodejs --out-dir out/node/$v target/wasm32-unknown-unknown/wasm-release/$lib.wasm
done
node scripts/intl-sample.cjs out/node/intl/w_intl.js | tee out/intl-sample.txt
