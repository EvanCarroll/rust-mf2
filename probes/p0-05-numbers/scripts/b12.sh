#!/usr/bin/env bash
# B12 symbol check + absolute size of numcore (± f64) in the no_std harness
# (bump allocator, panic handler ignores PanicInfo).
set -euo pipefail
cd "$(dirname "$0")/.."
OPT="--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue"
mkdir -p out
for feat in "" f64 loc; do
  tag=${feat:-nof64}
  CARGO_BUILD_JOBS=3 cargo build -q -p b12-numcore --target wasm32-unknown-unknown --profile wasm-syms ${feat:+--features $feat}
  f=target/wasm32-unknown-unknown/wasm-syms/b12_numcore.wasm
  csv=$(twiggy top -n 100000 --format csv $f)
  echo "[$tag] core::fmt symbols: $(grep -c 'core\[[0-9a-f]*\]::fmt' <<<"$csv" || true)"
  echo "[$tag] panic entry points: $(grep -cE '::panicking::|unwrap_failed|expect_failed|slice_index_fail' <<<"$csv" || true) ($(grep -E '::panicking::|unwrap_failed|expect_failed|slice_index_fail' <<<"$csv" | sed -E 's/\[[0-9a-f]+\]//g; s/,.*//' | sed 's/.*:://' | sort -u | tr '\n' ' '))"
  CARGO_BUILD_JOBS=3 cargo build -q -p b12-numcore --target wasm32-unknown-unknown --profile wasm-release ${feat:+--features $feat}
  wasm-opt -Oz $OPT target/wasm32-unknown-unknown/wasm-release/b12_numcore.wasm -o out/b12-numcore-$tag.opt.wasm
  imp=$(wasm-opt $OPT --print out/b12-numcore-$tag.opt.wasm 2>/dev/null | grep -c 'import "probe" "panic_reached"' || true)
  echo "[$tag] panic_reached import after LTO + wasm-opt: $imp (1 = a panic path is reachable)"
  strs=$(strings -n 6 out/b12-numcore-$tag.opt.wasm | grep -ciE 'unwrap|panick|out of bounds|overflow|\.rs' || true)
  echo "[$tag] panic message / source-path strings in the stripped wasm: $strs"
  echo "[$tag] numcore${feat:+ +$feat} (no_std harness): raw $(stat -c %s out/b12-numcore-$tag.opt.wasm) gz $(gzip -9 -c out/b12-numcore-$tag.opt.wasm | wc -c)"
done
