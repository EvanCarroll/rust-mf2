#!/usr/bin/env bash
# Phase 10 B1: 19 §14's `Display` (S3) / `Debug` (S2) row on the merged
# crate, with A9's cases (probes/p10-display-cost/case.py: one statement after
# the client's `install`, a working-tree edit, restored after each build):
#
# * shipped builds: demo-ssr `display-tr`, `tostring-tr`, `debug-trargs`;
#   demo-csr and demo-islands `debug-trargs` — against the b1 run's base
#   builds of the same demos (target/p10-b1/demos/b1/);
# * debug-profile builds of demo-ssr for fmt-check.sh: `base` (must pass),
#   `display-tr` and `debug-trargs` (must fail: the check can fail).
#
#   bash probes/p10-b1/display-debug.sh
set -u
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b1
logs=$out/logs/display-debug
mkdir -p "$logs"
export CARGO_NET_OFFLINE=true
case_py=probes/p10-display-cost/case.py

stamp() { echo "$(date -Is) $*" | tee -a "$logs/timeline.txt"; }

ship() {
  local d=$1 c=$2
  local dest=$out/demos/case-$c/$d
  rm -rf "$dest"; mkdir -p "$dest"
  python3 "$case_py" apply "$d" "$c" >>"$logs/case.log" 2>&1
  case $d in
    demo-ssr) (cd examples/$d && CARGO_BUILD_JOBS=2 cargo leptos build --release --split --frontend-only --cargo-offline) >"$logs/$d-$c.log" 2>&1 ;;
    demo-islands) (cd examples/$d && CARGO_BUILD_JOBS=2 cargo leptos build --release --frontend-only --cargo-offline) >"$logs/$d-$c.log" 2>&1 ;;
    demo-csr) (cd examples/$d && CARGO_BUILD_JOBS=3 trunk build --release) >"$logs/$d-$c.log" 2>&1 ;;
  esac
  local rc=$?
  python3 "$case_py" restore "$d" >>"$logs/case.log" 2>&1
  stamp "built $d $c rc=$rc"
  if [ "$d" = demo-csr ]; then src=examples/$d/dist; else src=examples/$d/target/site/pkg; fi
  cp -a "$src" "$dest/out"
  node probes/p10-names/measure-demo.mjs "$dest/out" "$d $c" >"$dest/measure.md"
  cat "$dest/measure.md" >>"$logs/timeline.txt"
}

dev() {
  local c=$1
  python3 "$case_py" apply demo-ssr "$c" >>"$logs/case.log" 2>&1
  (cd examples/demo-ssr && CARGO_TARGET_DIR="$out/t-demo-ssr-dev" CARGO_BUILD_JOBS=3 cargo build -q --lib \
    --target wasm32-unknown-unknown --no-default-features --features hydrate --offline) >"$logs/dev-$c.log" 2>&1
  local rc=$?
  python3 "$case_py" restore demo-ssr >>"$logs/case.log" 2>&1
  stamp "dev-built demo-ssr $c rc=$rc"
  mkdir -p "$out/dev-check"
  cp "$out/t-demo-ssr-dev/wasm32-unknown-unknown/debug/demo_ssr.wasm" "$out/dev-check/$c.wasm"
}

stamp "start"
for c in display-tr tostring-tr debug-trargs; do ship demo-ssr "$c"; done
ship demo-csr debug-trargs
ship demo-islands debug-trargs
for c in base display-tr debug-trargs; do dev "$c"; done
bash probes/p10-b1/fmt-check.sh "$out"/dev-check/*.wasm >"$logs/fmt-check.txt" 2>&1
cat "$logs/fmt-check.txt" >>"$logs/timeline.txt"
# The demos back to what the b1 run shipped (the locks were not touched).
stamp "done"
