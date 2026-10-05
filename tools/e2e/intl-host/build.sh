#!/usr/bin/env bash
# The intl-host check's browser modules (tools/e2e/checks/intl-host.mjs), one
# for each of the browser's number formatters:
#   target/e2e-intl-host/pkg-intl/, pkg-builtin/, pkg-plain/
#     — wasm-bindgen --target web
# Tools: cargo (+ wasm32-unknown-unknown), wasm-bindgen 0.2.128.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$repo/target/e2e-intl-host"
mkdir -p "$out"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
cd "$here"
for formatter in intl builtin plain; do
  cargo build -q --target wasm32-unknown-unknown --profile wasm-release \
    --no-default-features --features "$formatter"
  rm -rf "$out/pkg-$formatter"
  wasm-bindgen --target web --out-dir "$out/pkg-$formatter" \
    "target/wasm32-unknown-unknown/wasm-release/e2e_intl_host.wasm"
  echo "build.sh: $out/pkg-$formatter/"
done
