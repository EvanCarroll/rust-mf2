#!/usr/bin/env bash
# The D15 A/B (plans/10-phase-3-work-order.md A5b): the core numeric
# semantics over the own digit buffer (the default) against the same code
# over fixed_decimal (feature `fixed-decimal`), on P0.5's random corpus.
#
#  1. Output: `runtime-bench numbers corpus N` built both ways must print the
#     same lines (display, errors, cardinal and ordinal selection).
#  2. ECMA-402: P0.5's differential against node's Intl.NumberFormat (when
#     node is on PATH; a local tool, no network).
#  3. Speed and allocations: `numbers speed`, interleaved rounds.
#  4. Size and B12: bench/b12/check.sh (b12-runtime vs b12-runtime-fixed).
#
# Exit 1 if the outputs differ.
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
N="${N:-100000}"
OUT=target/number-ab
mkdir -p "$OUT"
cargo build -q --release -p runtime-bench
cargo build -q --release -p runtime-bench --features fixed-decimal --target-dir target/fixed-decimal
own=target/release/runtime-bench
fixed=target/fixed-decimal/release/runtime-bench

echo "== output: $N cases"
"$own" numbers corpus "$N" > "$OUT/own.tsv"
"$fixed" numbers corpus "$N" > "$OUT/fixed.tsv"
if cmp -s "$OUT/own.tsv" "$OUT/fixed.tsv"; then
  echo "  identical ($(wc -l < "$OUT/own.tsv") lines)"
else
  echo "  DIFFERENT:"; diff "$OUT/own.tsv" "$OUT/fixed.tsv" | head -20
  exit 1
fi

if command -v node >/dev/null 2>&1; then
  echo "== ECMA-402 differential (node Intl.NumberFormat('en', {useGrouping: false}))"
  "$own" numbers ecma "$N" | node bench/runtime-bench/ecma-diff.cjs | tail -1 | sed 's/^/  own:   /'
  "$fixed" numbers ecma "$N" | node bench/runtime-bench/ecma-diff.cjs | tail -1 | sed 's/^/  fixed: /'
fi

echo "== speed and allocations (interleaved; load average $(cut -d ' ' -f 1 /proc/loadavg))"
for _ in 1 2 3 4 5; do
  "$own" numbers speed 20000 --runs 21 | sed 's/^/  /'
  "$fixed" numbers speed 20000 --runs 21 | sed 's/^/  /'
done
