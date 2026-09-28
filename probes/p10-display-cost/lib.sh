#!/usr/bin/env bash
# A9's shared definitions, sourced by the other scripts. Run from the root of
# a tree on the A5 variant (branch p10-a5-display, or display.patch applied at
# 2fb7f54); outputs go under that tree's target/a9/.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
a9="$root/target/a9"
mkdir -p "$a9"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
# Rust 1.98's default target features for wasm32-unknown-unknown, as
# xtask/src/b5.rs passes them.
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
log() { echo "[$(date -Is)] $*" | tee -a "$a9/log.txt" >&2; }
