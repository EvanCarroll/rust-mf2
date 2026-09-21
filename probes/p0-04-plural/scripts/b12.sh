#!/usr/bin/env bash
# P0.4 B12 check (plans/06-size-and-perf.md §3): no core::fmt and no panic
# machinery reachable from the evaluator.
#  1. symbol names: twiggy over the non-stripped build (profile wasm-syms) and
#     over the same after wasm-opt -Oz --debuginfo;
#  2. panic reachability: the no_std harnesses' #[panic_handler] calls an
#     imported function; if any panic path survives optimisation, the module
#     imports `p04_panic_reachable`. Its absence proves the evaluator panic-free;
#  3. data strings: no panic / bounds / overflow message text in the module.
# For the std harness (which links std's own panic runtime) the check is that
# the evaluator adds no function to the std baseline's fmt/panic set (names
# compared without hashes and LLVM clone suffixes such as `.81`).
set -euo pipefail
cd "$(dirname "$0")/.."

T=target/wasm32-unknown-unknown/wasm-syms
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-syms \
  -p wasm-eval -p wasm-eval-ops -p wasm-std-alloc -p wasm-std-eval
mkdir -p out/b12

fmt_re='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|Formatter|Arguments|Debug|Display'
panic_re='panic|unwrap|expect|bounds|overflow|unreachable'
for c in eval eval-ops; do
  f="$T/wasm_${c//-/_}.wasm"
  o="out/b12/$c.syms.opt.wasm"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$f" -o "$o"
  echo "== wasm-$c (no_std harness)"
  for w in "$f" "$o"; do
    twiggy top -n 100000 --format csv "$w" > "out/b12/$(basename "$w").csv"
    printf '  %-40s functions/items %4d  fmt symbols %d  panic symbols %d\n' \
      "$(basename "$w")" \
      "$(($(wc -l < "out/b12/$(basename "$w").csv") - 1))" \
      "$(grep -cE "$fmt_re" "out/b12/$(basename "$w").csv" || true)" \
      "$(grep -ciE "$panic_re" "out/b12/$(basename "$w").csv" || true)"
  done
  printf '  import p04_panic_reachable present: %s\n' \
    "$(grep -qa p04_panic_reachable "$o" && echo YES || echo no)"
  printf '  panic/bounds/overflow strings in data: %d\n' \
    "$(strings -n 5 "out/wasm/$c.opt.wasm" | grep -ciE 'panic|bounds|overflow|unwrap|called `' || true)"
  echo "  items (twiggy top, optimised):"
  twiggy top -n 12 "$o" | sed 's/^/    /'
done

echo "== std harness: symbols added by the evaluator to the fair std baseline (std-alloc)"
for c in std-alloc std-eval; do
  twiggy top -n 100000 --format csv "$T/wasm_${c//-/_}.wasm" | cut -d, -f1 | sed -E 's/::h[0-9a-f]{16}//; s/\.[0-9]+$//' | sort -u > "out/b12/$c.names"
done
comm -13 out/b12/std-alloc.names out/b12/std-eval.names | sed 's/^/  + /'
echo "  of which fmt/panic: $(comm -13 out/b12/std-alloc.names out/b12/std-eval.names | grep -ciE "$fmt_re|$panic_re" || true)"
