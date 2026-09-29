#!/usr/bin/env bash
# Phase 10 C2: a size workload's client with its symbol names kept, at
# HEAD's sources and at C2's, for a function-level reading of what C2 moved
# (C1's `probes/p10-c1/named.sh`, over `at-head.sh`'s swap: every file git
# sees as changed or new under `crates/`).
#
#   bash probes/p10-c2/named.sh [TEMPLATE [WORKLOAD]]
#
# TEMPLATE is `tr` (the default), `tr-view`, or a demo (`demo-islands`,
# `demo-ssr`, built against its base lock); WORKLOAD the kept workload
# directory (default: `size`'s at 1,860 sites for `tr`, `b5 --view`'s for
# `tr-view`). Run from the main tree with C2's change in place, after
# `bash probes/p10-b2/measure.sh c2`. Outputs in target/p10-c2/named/; its
# build directory `t` is removed at the end.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
template=${1:-tr}
case $template in
  tr) app=${2:-$root/target/p10-b2/size/wl-1860}/app-tr ;;
  demo-*) app=$root/examples/$template ;;
  *) app=${2:-$root/target/p10-b2/b5v/wl-view-1860}/app-$template ;;
esac
case $template in
  demo-*) lib=${template//-/_}; workload=$root ;;
  *) lib=workload_app_${template//-/_}; workload=$(dirname "$app") ;;
esac
out=$root/target/p10-c2/named/$template
export CARGO_NET_OFFLINE=true
mkdir -p "$out"

build() {
  local label=$1
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
export -f build
export template app lib workload out root
bash probes/p10-c2/at-head.sh bash -c 'build base'
build c2
python3 probes/p10-b1/norm-diff.py "$out/base.json" "$out/c2.json" 60 >"$out/norm-diff.txt"
rm -rf "$out/t"
echo done
