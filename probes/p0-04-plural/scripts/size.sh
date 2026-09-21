#!/usr/bin/env bash
# P0.4 wasm size measurement, plans/06-size-and-perf.md §3 method:
# wasm32-unknown-unknown, profile wasm-release (opt-level z, fat LTO, 1 CGU,
# panic abort, strip) -> wasm-opt -Oz -> gzip -9. Every figure is also given as
# a delta against its base crate (same scaffolding, no plural code).
# Run from anywhere; writes out/wasm/*.opt.wasm and out/size.tsv.
set -euo pipefail
cd "$(dirname "$0")/.."

T=target/wasm32-unknown-unknown/wasm-release
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CRATES=(base eval base-ops eval-ops audit-hand std-base std-alloc std-eval icu-compiled icu-blob)

args=()
for c in "${CRATES[@]}"; do args+=(-p "wasm-$c"); done
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-release "${args[@]}"

mkdir -p out/wasm
declare -A RAW GZ
for c in "${CRATES[@]}"; do
  in="$T/wasm_${c//-/_}.wasm"
  out="out/wasm/$c.opt.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$in" -o "$out"
  RAW[$c]=$(stat -c %s "$out")
  GZ[$c]=$(gzip -9 -n -c "$out" | wc -c)
done

printf '%-14s %8s %8s   %-10s %9s %9s\n' crate raw gz base "Δraw" "Δgz"
{
  printf 'crate\traw\tgz\tbase\tdelta_raw\tdelta_gz\n'
  for pair in base: eval:base base-ops: eval-ops:base-ops audit-hand:base-ops std-base: std-alloc:std-base \
              std-eval:std-base icu-compiled:std-base icu-blob:std-base \
              std-eval:std-alloc icu-compiled:std-alloc icu-blob:std-alloc; do
    c=${pair%%:*}; b=${pair#*:}
    if [ -n "$b" ]; then
      dr=$(( RAW[$c] - RAW[$b] )); dg=$(( GZ[$c] - GZ[$b] ))
      printf '%-14s %8d %8d   %-10s %9d %9d\n' "$c" "${RAW[$c]}" "${GZ[$c]}" "$b" "$dr" "$dg" >&2
      printf '%s\t%d\t%d\t%s\t%d\t%d\n' "$c" "${RAW[$c]}" "${GZ[$c]}" "$b" "$dr" "$dg"
    else
      printf '%-14s %8d %8d   %-10s %9s %9s\n' "$c" "${RAW[$c]}" "${GZ[$c]}" "-" "-" "-" >&2
      printf '%s\t%d\t%d\t-\t-\t-\n' "$c" "${RAW[$c]}" "${GZ[$c]}"
    fi
  done
} > out/size.tsv
