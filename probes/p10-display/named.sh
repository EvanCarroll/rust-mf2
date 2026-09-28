#!/usr/bin/env bash
# A5: the measured apps again, keeping symbol names (strip = false), for
# twiggy. Same workloads, same crates, same pipeline as `cargo xtask b5`
# (wasm-bindgen, then wasm-opt -Oz, here with --debuginfo so the names
# survive). Not a size measurement: the name section is extra bytes.
set -euo pipefail
label="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
export CARGO_BUILD_JOBS=2
out="$root/target/a5/named/$label"
mkdir -p "$out"
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)

app() {
  local kind="$1" wl="$2" template="$3"
  local workload="$root/target/$kind/$wl"
  local lib="workload_app_${template//-/_}"
  local target="$root/target/a5/named/target-$kind-$wl"
  echo "named: $label $kind/$wl/app-$template $(date -Is)"
  (cd "$workload/app-$template" && \
    CARGO_TARGET_DIR="$target" MF2_WORKLOAD_LOCALES="$workload" \
    CARGO_PROFILE_WASM_RELEASE_STRIP=false \
    nice -n 10 cargo build --quiet --lib --no-default-features --features hydrate \
      --target wasm32-unknown-unknown --profile wasm-release)
  local dest="$out/$kind-$wl-$template"
  rm -rf "$dest"
  mkdir -p "$dest"
  wasm-bindgen --target web --no-typescript --out-dir "$dest" \
    "$target/wasm32-unknown-unknown/wasm-release/$lib.wasm"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$dest/${lib}_bg.wasm" -o "$dest/opt.wasm"
}

fixture() {
  local features="$1"
  local target="$root/target/a5/named/target-fixture"
  echo "named: $label fixture $features $(date -Is)"
  CARGO_TARGET_DIR="$target" CARGO_PROFILE_WASM_RELEASE_STRIP=false \
    nice -n 10 cargo build --quiet -p mf2-i18n-fixture --bin mf2-i18n-client --no-default-features \
      --features "$features" --target wasm32-unknown-unknown --profile wasm-release
  local dest="$out/fixture-${features//,/+}"
  rm -rf "$dest"
  mkdir -p "$dest"
  cp "$target/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" "$dest/raw.wasm"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$dest/raw.wasm" -o "$dest/opt.wasm"
}

# The view workload is generated once, by workload-gen with b5 --view's
# arguments, into target/a5wl (so the measured target/b5 is left alone).
app a5wl wl-view-1860 tr-view
app size wl-1860 tr
fixture hydrate,fn-number
echo "named: $label done $(date -Is)"
