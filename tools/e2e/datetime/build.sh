#!/usr/bin/env bash
# The datetime-intl browser comparison's inputs (plans/11 A6):
#   target/e2e-datetime/cases.json  — every case's catalog and ICU4X's text
#   target/e2e-datetime/pkg/        — the browser module (wasm-bindgen --target web)
# Tools: cargo (+ wasm32-unknown-unknown), wasm-bindgen 0.2.128.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$repo/target/e2e-datetime"
mkdir -p "$out"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
cd "$here"
cargo run -q --release -p e2e-datetime-cases -- "$out/cases.json"
cargo build -q --target wasm32-unknown-unknown --profile wasm-release -p e2e-datetime-wasm
wasm-bindgen --target web --out-dir "$out/pkg" \
  "target/wasm32-unknown-unknown/wasm-release/e2e_datetime_wasm.wasm"
echo "build.sh: $out/cases.json, $out/pkg/"
