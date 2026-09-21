#!/usr/bin/env bash
# P0.3 B12 check (plans/06 §3): no core::fmt and no panic machinery reachable
# from the runtime.
#  1. symbol names: twiggy over the non-stripped build (profile wasm-syms) and
#     over the same after wasm-opt -Oz --debuginfo;
#  2. panic reachability: the no_std harness's #[panic_handler] calls the
#     imported `p03_panic_reachable`; if no panic path survives LTO + wasm-opt
#     the import is absent. The harness itself is panic-free (fixed output
#     buffer, raw allocator export), so a surviving import is the runtime's;
#  3. data strings: no panic / bounds / overflow message text.
set -euo pipefail
cd "$(dirname "$0")/.."
T=target/wasm32-unknown-unknown/wasm-syms
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-syms -p wasm-floor -p wasm-base
mkdir -p out/b12
fmt_re='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|fmt::Formatter|fmt::Arguments|Debug|Display'
panic_re='panic|unwrap|expect|bounds|overflow|unreachable|capacity|handle_alloc_error|oom'
for c in base floor; do
  f="$T/wasm_${c}.wasm"; o="out/b12/$c.syms.opt.wasm"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$f" -o "$o"
  echo "== wasm-$c (no_std harness)"
  for w in "$f" "$o"; do
    twiggy top -n 100000 --format csv "$w" > "out/b12/$(basename "$w").csv"
    printf '  %-26s items %4d  fmt symbols %d  panic/alloc-failure symbols %d\n' "$(basename "$w")" \
      "$(($(wc -l < "out/b12/$(basename "$w").csv") - 1))" \
      "$(grep -cE "$fmt_re" "out/b12/$(basename "$w").csv" || true)" \
      "$(grep -ciE "$panic_re" "out/b12/$(basename "$w").csv" || true)"
  done
  printf '  import p03_panic_reachable present after LTO + wasm-opt: %s\n' "$(grep -qa p03_panic_reachable "$o" && echo YES || echo no)"
  printf '  panic/bounds/overflow strings in the stripped optimised module: %d\n' \
    "$(strings -n 5 "out/wasm/$c.opt.wasm" 2>/dev/null | grep -ciE 'panic|bounds|overflow|unwrap|called `|capacity' || true)"
  echo "  fmt/panic symbols (non-stripped, pre-wasm-opt):"
  grep -iE "$fmt_re|$panic_re" "out/b12/$(basename "$f").csv" | cut -d, -f1 | sed 's/^/    /' | head -20 || true
done
echo "== wasm-floor with --features p03-rt/utf8-per-access"
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-syms -p wasm-floor --features p03-rt/utf8-per-access
wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$T/wasm_floor.wasm" -o out/b12/floor-pa.syms.opt.wasm
twiggy top -n 100000 --format csv out/b12/floor-pa.syms.opt.wasm > out/b12/floor-pa.csv
printf '  fmt symbols %d  panic/alloc-failure symbols %d  import p03_panic_reachable present: %s\n' \
  "$(grep -cE "$fmt_re" out/b12/floor-pa.csv || true)" "$(grep -ciE "$panic_re" out/b12/floor-pa.csv || true)" \
  "$(grep -qa p03_panic_reachable out/b12/floor-pa.syms.opt.wasm && echo YES || echo no)"
CARGO_BUILD_JOBS=3 cargo build -q --target wasm32-unknown-unknown --profile wasm-syms -p wasm-floor
echo "== largest items in the optimised floor module"
twiggy top -n 25 out/b12/floor.syms.opt.wasm | sed 's/^/  /'
