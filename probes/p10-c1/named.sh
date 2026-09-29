#!/usr/bin/env bash
# Phase 10 C1: a size workload's client with its symbol names kept, at
# HEAD's sources and at C1's, for a function-level reading of what C1 moved
# (B1's method: cargo's own build, before wasm-bindgen and wasm-opt; twiggy;
# probes/p10-b1/norm-diff.py).
#
#   bash probes/p10-c1/named.sh [TEMPLATE [WORKLOAD]]
#
# TEMPLATE is `tr-view` (the default), `tr`, or a demo (`demo-islands`,
# `demo-ssr`, built against its base lock); WORKLOAD the kept workload
# directory (default: `b5 --view`'s at 1,860 sites, or `size`'s for `tr`).
# Run from the main tree with C1's change in place, after
# `bash probes/p10-b2/measure.sh c1` (which leaves the workloads and their
# locks in target/p10-b2/). C1's crate sources are kept aside, HEAD's
# written in their place for the first build, and C1's put back (with fresh
# modification times, so that cargo rebuilds) for the second, whatever
# happens. Outputs in target/p10-c1/named/; its build directory `t` is
# removed at the end.
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
  demo-*) lib=${template//-/_}; workload=$root ;;
  *) lib=workload_app_${template//-/_}; workload=$(dirname "$app") ;;
esac
out=$root/target/p10-c1/named/$template
changed="crates/mf2/src/lib.rs crates/mf2/src/leptos/signal.rs crates/mf2-macros/src/expand.rs"
added="crates/mf2/src/into_arg.rs crates/mf2/src/__arg.rs"
export CARGO_NET_OFFLINE=true
mkdir -p "$out"
tar -cf "$out/c1-src.tar" $changed $added
restore() { tar -xmf "$out/c1-src.tar"; }
trap restore EXIT

build() {
  local label=$1
  # A demo builds against the lock its size runs restore.
  case $template in
    demo-*) cp "$root/target/p10-b2/locks/base/$template.Cargo.lock" "$app/Cargo.lock" ;;
  esac
  (cd "$app" && CARGO_PROFILE_WASM_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 \
    CARGO_TARGET_DIR="$out/t" MF2_WORKLOAD_LOCALES="$workload" cargo build --lib \
    --no-default-features --features hydrate --target wasm32-unknown-unknown \
    --profile wasm-release --offline) >"$out/$label.log" 2>&1
  cp "$out/t/wasm32-unknown-unknown/wasm-release/$lib.wasm" "$out/$label.wasm"
  twiggy top -n 1000000 -f json "$out/$label.wasm" >"$out/$label.json"
}

for f in $changed; do git show "HEAD:$f" >"$f"; done
rm -f $added
build base
restore
build c1
python3 probes/p10-b1/norm-diff.py "$out/base.json" "$out/c1.json" 60 >"$out/norm-diff.txt"
rm -rf "$out/t"
echo done
