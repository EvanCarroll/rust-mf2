#!/usr/bin/env bash
# P0.3 wasm size (plans/06 §3 method): wasm32-unknown-unknown, profile
# wasm-release (opt-level z, fat LTO, 1 CGU, panic abort, strip) -> wasm-opt -Oz
# -> gzip -9, as deltas against base crates with the same scaffolding/exports.
# Writes out/wasm/*.opt.wasm and out/size.tsv.
set -euo pipefail
cd "$(dirname "$0")/.."
T=target/wasm32-unknown-unknown/wasm-release
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CRATES=(base floor std-base std-floor)
args=(); for c in "${CRATES[@]}"; do args+=(-p "wasm-$c"); done
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-release "${args[@]}"
mkdir -p out/wasm
declare -A RAW GZ BR
for c in "${CRATES[@]}"; do
  in="$T/wasm_${c//-/_}.wasm"; out="out/wasm/$c.opt.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$in" -o "$out"
  RAW[$c]=$(stat -c %s "$out"); GZ[$c]=$(gzip -9 -n -c "$out" | wc -c)
done
# The same floors with P0.8's recommended UTF-8 strategy (per string on access,
# zero-copy load).
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-release -p wasm-floor -p wasm-std-floor --features p03-rt/utf8-per-access
for c in floor std-floor; do
  in="$T/wasm_${c//-/_}.wasm"; out="out/wasm/$c-pa.opt.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$in" -o "$out"
  RAW[$c-pa]=$(stat -c %s "$out"); GZ[$c-pa]=$(gzip -9 -n -c "$out" | wc -c)
done
{
  printf 'crate\traw\tgz\tbase\tdelta_raw\tdelta_gz\n'
  for pair in base: floor:base floor-pa:base std-base: std-floor:std-base std-floor-pa:std-base; do
    c=${pair%%:*}; b=${pair#*:}
    if [ -n "$b" ]; then
      printf '%s\t%d\t%d\t%s\t%d\t%d\n' "$c" "${RAW[$c]}" "${GZ[$c]}" "$b" $((RAW[$c]-RAW[$b])) $((GZ[$c]-GZ[$b]))
    else
      printf '%s\t%d\t%d\t-\t-\t-\n' "$c" "${RAW[$c]}" "${GZ[$c]}"
    fi
  done
} | tee out/size.tsv | column -t
