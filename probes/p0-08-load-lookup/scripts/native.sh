#!/usr/bin/env bash
# P0.8 native: allocation counts + criterion, for both UTF-8 strategies.
# Writes out/allocs-{eager,per-access}.md and out/criterion-{eager,per-access}.txt.
set -euo pipefail
cd "$(dirname "$0")/.."
for v in eager per-access; do
  feat=(); [ "$v" = per-access ] && feat=(--features utf8-per-access)
  CARGO_BUILD_JOBS=3 cargo build -q --release -p p08-bench "${feat[@]}"
  ./target/release/p08-allocs --emit out/web > "out/allocs-$v.md"
  CARGO_BUILD_JOBS=3 cargo bench -q -p p08-bench --bench load_lookup "${feat[@]}" -- \
    --warm-up-time 1 --measurement-time 3 --noplot > "out/criterion-$v.txt" 2>&1
done
for v in eager per-access; do
  echo "== $v"
  grep -E "time:" -B1 "out/criterion-$v.txt" | grep -vE "^--|Analyzing|Benchmarking" | paste - - | sed -E 's/ +/ /g'
done
