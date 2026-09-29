#!/usr/bin/env bash
# Phase 10 C3: a client with its symbol names kept, at HEAD's sources and at
# C3's, for a function-level reading of what C3 moved (C2's
# `probes/p10-c2/named.sh`, over C3's `at-head.sh`, and demo-csr too).
#
#   bash probes/p10-c3/named.sh [TEMPLATE [WORKLOAD]]
#
# TEMPLATE is `tr-view` (the default, `b5 --view`'s kept workload at 1,860
# sites), `tr`, `demo-islands` or `demo-ssr` (hydrate, built against their
# base lock), or `demo-csr` (its binary, `csr`, built against its base
# lock). Run from the main tree after `bash probes/p10-b2/measure.sh c3`.
# Outputs in target/p10-c3/named/TEMPLATE/: each build's wasm (before
# wasm-bindgen and wasm-opt), its twiggy table, and `norm-diff.txt`; the
# build directory `t` is removed at the end.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
template=${1:-tr-view}
case $template in
  tr) app=${2:-$root/target/p10-b2/size/wl-1860}/app-tr ;;
  demo-*) app=$root/examples/$template ;;
  *) app=${2:-$root/target/p10-b2/b5v/wl-view-1860}/app-$template ;;
esac
case $template in
  demo-csr) lib=demo-csr; workload=$root ;;
  demo-*) lib=${template//-/_}; workload=$root ;;
  *) lib=workload_app_${template//-/_}; workload=$(dirname "$app") ;;
esac
out=$root/target/p10-c3/named/$template
export CARGO_NET_OFFLINE=true
mkdir -p "$out"

build() {
  local label=$1
  case $template in
    demo-*) cp "$root/target/p10-b2/locks/base/$template.Cargo.lock" "$app/Cargo.lock" ;;
  esac
  case $template in
    demo-csr)
      (cd "$app" && CARGO_PROFILE_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 \
        CARGO_TARGET_DIR="$out/t" cargo build --bin demo-csr \
        --target wasm32-unknown-unknown --release --offline) >"$out/$label.log" 2>&1
      cp "$out/t/wasm32-unknown-unknown/release/demo-csr.wasm" "$out/$label.wasm" ;;
    *)
      (cd "$app" && CARGO_PROFILE_WASM_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 \
        CARGO_TARGET_DIR="$out/t" MF2_WORKLOAD_LOCALES="$workload" cargo build --lib \
        --no-default-features --features hydrate --target wasm32-unknown-unknown \
        --profile wasm-release --offline) >"$out/$label.log" 2>&1
      cp "$out/t/wasm32-unknown-unknown/wasm-release/$lib.wasm" "$out/$label.wasm" ;;
  esac
  twiggy top -n 1000000 -f json "$out/$label.wasm" >"$out/$label.json"
}
export -f build
export template app lib workload out root
bash probes/p10-c3/at-head.sh bash -c 'build base'
build c3
python3 probes/p10-b1/norm-diff.py "$out/base.json" "$out/c3.json" 60 >"$out/norm-diff.txt"
rm -rf "$out/t"
echo done
