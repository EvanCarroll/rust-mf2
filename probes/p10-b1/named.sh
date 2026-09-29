#!/usr/bin/env bash
# Phase 10 B1: demo-ssr's client with its symbol names kept, at base and at
# B1 (a patch of the B1 change, applied and reverted in the tree), for a
# function-level reading of the move (A7's method: cargo's own build, before
# wasm-bindgen and wasm-opt; twiggy; probes/p10-names/twiggy-norm-diff.py).
#
#   bash probes/p10-b1/named.sh probes/p10-b1/b1-static.patch
#
# From the root of B1's measurement tree, where that patch is applied:
# `b1-static.patch` is B1's change as that tree holds it (the code is
# 1023d57's; only comments differ), reversed for the base build and applied
# again for B1's.
set -eu
patch=${1:?the B1 patch}
cd "$(dirname "$0")/../.."
out=$(pwd)/target/p10-b1
export CARGO_NET_OFFLINE=true
build() {
  local label=$1
  cp "$out/locks/base/demo-ssr.Cargo.lock" examples/demo-ssr/Cargo.lock
  (cd examples/demo-ssr && CARGO_PROFILE_WASM_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 cargo build --lib \
    --target wasm32-unknown-unknown --profile wasm-release --no-default-features --features hydrate \
    --target-dir "$out/named/t-$label" --offline) >"$out/named/$label.log" 2>&1
  cp "$out/named/t-$label/wasm32-unknown-unknown/wasm-release/demo_ssr.wasm" "$out/named/$label.wasm"
  twiggy top -n 1000000 -f json "$out/named/$label.wasm" >"$out/named/$label.json"
}
mkdir -p "$out/named"
git apply -R --binary "$patch"
git status --short >"$out/named/base-status.txt"
build base
git apply --binary "$patch"
build b1
python3 probes/p10-b1/norm-diff.py "$out/named/base.json" "$out/named/b1.json" 60 >"$out/named/norm-diff.txt"
echo done
