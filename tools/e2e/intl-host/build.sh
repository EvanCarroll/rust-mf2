#!/usr/bin/env bash
# The intl-host check's browser module (tools/e2e/checks/intl-host.mjs):
#   target/e2e-intl-host/pkg/  — wasm-bindgen --target web
# Tools: cargo (+ wasm32-unknown-unknown), wasm-bindgen 0.2.128.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$repo/target/e2e-intl-host"
mkdir -p "$out"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
cd "$here"
cargo build -q --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --out-dir "$out/pkg" \
  "target/wasm32-unknown-unknown/wasm-release/e2e_intl_host.wasm"
echo "build.sh: $out/pkg/"
