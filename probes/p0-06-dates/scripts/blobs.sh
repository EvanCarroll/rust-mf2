#!/usr/bin/env bash
# Export per-locale ICU4X blobs restricted to the markers each wasm variant
# requests. Needs the wasm-release builds (scripts/measure.sh icu-blob-fixed
# icu-blob-fixed-nozone) first.
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BUILD_JOBS=3 cargo build -q -p blobgen --release
B=./target/release/blobgen
W=target/wasm32-unknown-unknown/wasm-release
mkdir -p out/blobs
for loc in en ar ja ru; do
  $B export --bin $W/w_icu_blob_fixed.wasm        --locale $loc --out out/blobs/fixed-$loc.postcard
  $B export --bin $W/w_icu_blob_fixed_nozone.wasm --locale $loc --out out/blobs/nozone-$loc.postcard
  # A corpus using only default `:date`/`:time`/`:datetime`: skeletons ym0d + j, glue mdt.
  $B export --bin $W/w_icu_blob_fixed_nozone.wasm --locale $loc --attrs ym0d,mdt,j --out out/blobs/date1-$loc.postcard
done
for f in out/blobs/*.postcard; do
  printf '%-32s raw %7d gz %7d\n' "$(basename "$f")" "$(stat -c %s "$f")" "$(gzip -9 -c "$f" | wc -c)"
done
