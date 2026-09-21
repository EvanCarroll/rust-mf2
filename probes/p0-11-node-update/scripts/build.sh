#!/usr/bin/env bash
# P0.11: build the native model and both browser harness variants.
set -euo pipefail
probe=$(realpath "$(dirname "$0")/..")
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
export CARGO_TARGET_DIR=$probe/target/cargo
(cd "$probe/native" && cargo build --release --quiet)
for s in a b; do
  feat=(); [ "$s" = a ] && feat=(--features a)
  (cd "$probe/wasm" && cargo build --quiet --lib --target wasm32-unknown-unknown --profile wasm-release "${feat[@]}")
  rm -rf "$probe/target/pkg-$s"
  wasm-bindgen --target web --no-typescript --out-dir "$probe/target/pkg-$s" \
      "$CARGO_TARGET_DIR/wasm32-unknown-unknown/wasm-release/p0_11_wasm.wasm"
done
ls -l "$probe"/target/pkg-*/p0_11_wasm_bg.wasm
