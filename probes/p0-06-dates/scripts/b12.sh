#!/usr/bin/env bash
# B12 symbol check for mf2dt: no_std cdylib, panic handler ignores PanicInfo,
# non-stripped build; count core::fmt and panic symbols with twiggy.
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BUILD_JOBS=3 cargo build -q -p b12-mf2dt --target wasm32-unknown-unknown --profile wasm-syms
f=target/wasm32-unknown-unknown/wasm-syms/b12_mf2dt.wasm
echo "core::fmt symbols: $(twiggy top -n 100000 --format csv $f | grep -c 'core\[[0-9a-f]*\]::fmt' || true)"
echo "panic symbols:     $(twiggy top -n 100000 --format csv $f | grep -ci 'panic' || true)"
CARGO_BUILD_JOBS=3 cargo build -q -p b12-mf2dt --target wasm32-unknown-unknown --profile wasm-release
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  target/wasm32-unknown-unknown/wasm-release/b12_mf2dt.wasm -o out/b12-mf2dt.opt.wasm
echo "mf2dt alone (no_std harness): raw $(stat -c %s out/b12-mf2dt.opt.wasm) gz $(gzip -9 -c out/b12-mf2dt.opt.wasm | wc -c)"
