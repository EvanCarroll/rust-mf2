#!/usr/bin/env bash
# Phase 10 B1: `{:?}` on a `TrArgs` in the two other clients A9 measured S2
# in — the fixture client (as `cargo xtask b12-generated` builds its A, then
# `wasm-opt -Oz`) and `tr-view` at 1,860 sites (as `b5 --view` builds it) —
# each against its own base, `gzip -9 -n` of the optimised module (A9's
# measure).
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b1
logs=$out/logs/display-debug
export CARGO_NET_OFFLINE=true
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
gz() { gzip -9 -n -c "$1" | wc -c; }
fixture() {
  local label=$1
  CARGO_TARGET_DIR="$out/t-fixture" CARGO_BUILD_JOBS=3 cargo build --quiet -p mf2-i18n-fixture \
    --bin mf2-i18n-client --no-default-features --features hydrate,fn-number \
    --target wasm32-unknown-unknown --profile wasm-release
  mkdir -p "$out/fixture/$label"
  wasm-opt -Oz "${FEATURES[@]}" "$out/t-fixture/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" \
    -o "$out/fixture/$label/opt.wasm"
  echo "fixture $label: $(stat -c %s "$out/fixture/$label/opt.wasm") raw, $(gz "$out/fixture/$label/opt.wasm") gzip -9 -n" | tee -a "$logs/timeline.txt"
}
fixture base
python3 probes/p10-display-cost/case.py apply fixture debug-trargs
fixture debug-trargs
python3 probes/p10-display-cost/case.py restore fixture

wl=$out/b5v/wl-view-1860
app=$wl/app-tr-view
lib=workload_app_tr_view
cp "$app/src/lib.rs" "$out/tr-view-lib.rs.pristine"
tview() {
  local label=$1
  (cd "$app" && CARGO_TARGET_DIR="$wl/target-apps" MF2_WORKLOAD_LOCALES="$wl" CARGO_BUILD_JOBS=3 \
    cargo build --quiet --lib --no-default-features --features hydrate --target wasm32-unknown-unknown \
    --profile wasm-release)
  local dest=$out/tr-view/$label
  rm -rf "$dest"; mkdir -p "$dest/pkg"
  wasm-bindgen --target web --no-typescript --out-dir "$dest/pkg" "$wl/target-apps/wasm32-unknown-unknown/wasm-release/$lib.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$dest/pkg/${lib}_bg.wasm" -o "$dest/opt.wasm"
  echo "tr-view $label: $(stat -c %s "$dest/opt.wasm") raw, $(gz "$dest/opt.wasm") gzip -9 -n" | tee -a "$logs/timeline.txt"
}
tview base
python3 - "$app/src/lib.rs" <<'PY'
import sys
p = sys.argv[1]
t = open(p).read()
anchor = "    crate::support::boot();\n"
assert t.count(anchor) == 1
t = t.replace(anchor, anchor + "    // B1 case (a working-tree edit, never kept)\n    {\n        let x = std::hint::black_box(workload_i18n::tr!(\"admin.labels.disabled\", \"item\" = \"Ada\"));\n        std::hint::black_box(format!(\"{:?}\", x));\n    }\n")
open(p, "w").write(t)
PY
tview debug-trargs
cp "$out/tr-view-lib.rs.pristine" "$app/src/lib.rs"
echo done
