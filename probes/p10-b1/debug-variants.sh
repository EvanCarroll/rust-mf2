#!/usr/bin/env bash
# Phase 10 B1: the two leaner forms of the `Debug` writers that B1's record
# names beside the kept one (S2, `crates/mf2/src/debug.rs`), in the client
# that formats nothing else: demo-islands with A9's `debug-trargs` case
# (probes/p10-display-cost/case.py: `format!("{:?}", …)` on
# `tr!("people-online", count = 3)`), built as it ships.
#
# * `debug-writestr.patch` — `write_str` for every character, `write_char`
#   nowhere (the digits from a table of one-digit strings);
# * `debug-buffer.patch` — each number's digits into a buffer, then one
#   `write_str`.
#
# The variants' code was not kept when B1 measured them; these patches are
# their reconstruction (plans/18-phase-10-work-order.md, B1's review fixes).
#
#   bash probes/p10-b1/debug-variants.sh
#
# Run from the root of B1's measurement tree (README.md) after `measure.sh
# b1`, or with this script and the two patches copied into that tree's
# target/p10-b1/ (as its other scripts ran): it finds the patches beside
# itself. Each patch is applied to the working tree and reverted after its
# build. Outputs in target/p10-b1/demos/variant-<label>/ and
# target/p10-b1/logs/variants/.
set -u
here=$(cd "$(dirname "$0")" && pwd)
cd "$here/../.."
out=$(pwd)/target/p10-b1
logs=$out/logs/variants
mkdir -p "$logs"
export CARGO_NET_OFFLINE=true
case_py=probes/p10-display-cost/case.py

build() {
  local label=$1 patch=${2:-}
  if [ -n "$patch" ]; then git apply "$here/$patch" || exit 1; fi
  python3 "$case_py" apply demo-islands debug-trargs >>"$logs/case.log" 2>&1
  (cd examples/demo-islands && CARGO_BUILD_JOBS=2 cargo leptos build --release --frontend-only --cargo-offline) >"$logs/$label.log" 2>&1
  local rc=$?
  python3 "$case_py" restore demo-islands >>"$logs/case.log" 2>&1
  if [ -n "$patch" ]; then git apply -R "$here/$patch" || exit 1; fi
  echo "built $label rc=$rc"
  local dest=$out/demos/variant-$label/demo-islands
  rm -rf "$dest"; mkdir -p "$dest"
  cp -a examples/demo-islands/target/site/pkg "$dest/out"
  node probes/p10-names/measure-demo.mjs "$dest/out" "demo-islands debug-trargs, $label" | tee "$dest/measure.md"
}

build s2
build writestr debug-writestr.patch
build buffer debug-buffer.patch
