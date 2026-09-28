#!/usr/bin/env bash
# A9: build one client for each case and measure what ships.
#
#   run.sh CLIENT CASE...            (from the root of the tree)
#
# CLIENT is fixture, tr, tr-view, demo-ssr, demo-csr or demo-islands; a CASE
# is one of case.py's. Each case is applied, built, measured and taken out
# again, so the tree is as it was after every case. Environment:
#   A9_LIB    the library variant's label, recorded with each figure (default
#             a5: the probe branch as it stands; the others are patches)
#   A9_NAMED  1: also a build with symbol names kept, for twiggy
#   A9_SHIP   0: skip the shipped build (only with A9_NAMED=1)
#
# The shipped build is each client's own pipeline: the fixture as
# `cargo xtask b12-generated` builds its A (hydrate,fn-number) then
# `wasm-opt -Oz`; `tr` and `tr-view` as `cargo xtask size` / `b5 --view` build
# them (wasm-bindgen, then wasm-opt -Oz), in their warm target directories;
# the demos as A7 measured them (`cargo leptos build --release [--split]
# --frontend-only`, `trunk build --release`). The named build keeps names
# (strip = false), and `wasm-opt --strip-dwarf -Oz --debuginfo` drops the
# standard library's DWARF first, so that its code is what ships (A9 found
# DWARF left in makes binaryen emit +557 B of code in the fixture).
source "$(dirname "$0")/lib.sh"
client="$1"
shift
lib="${A9_LIB:-a5}"
named="${A9_NAMED:-0}"
ship="${A9_SHIP:-1}"
cd "$root"

wasm_opt_named() { wasm-opt --strip-dwarf -Oz --debuginfo "${FEATURES[@]}" "$1" -o "$2"; }

build_fixture() {
  local dest="$1"
  local args=(-p mf2-i18n-fixture --bin mf2-i18n-client --no-default-features --features hydrate,fn-number
    --target wasm32-unknown-unknown --profile wasm-release)
  if [ "$ship" = 1 ]; then
    local t="$a9/t-fixture-stripped"
    CARGO_TARGET_DIR="$t" nice -n 10 cargo build --quiet "${args[@]}" || return 1
    mkdir -p "$dest/ship"
    cp "$t/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" "$dest/raw.wasm"
    wasm-opt -Oz "${FEATURES[@]}" "$dest/raw.wasm" -o "$dest/ship/opt.wasm"
  fi
  if [ "$named" = 1 ]; then
    local t="$root/target/a5/named/target-fixture"
    CARGO_TARGET_DIR="$t" CARGO_PROFILE_WASM_RELEASE_STRIP=false nice -n 10 cargo build --quiet "${args[@]}" || return 1
    wasm_opt_named "$t/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" "$dest/named.wasm"
  fi
}

# A workload app: $1 dest, $2 workload dir, $3 template, $4 named target dir.
build_workload() {
  local dest="$1" wl="$2" template="$3" named_t="$4"
  local lib="workload_app_${template//-/_}"
  local args=(--lib --no-default-features --features hydrate --target wasm32-unknown-unknown
    --profile wasm-release)
  if [ "$ship" = 1 ]; then
    local t="$wl/target-apps"
    (cd "$wl/app-$template" && CARGO_TARGET_DIR="$t" MF2_WORKLOAD_LOCALES="$wl" \
      nice -n 10 cargo build --quiet "${args[@]}") || return 1
    mkdir -p "$dest/pkg" "$dest/ship"
    wasm-bindgen --target web --no-typescript --out-dir "$dest/pkg" \
      "$t/wasm32-unknown-unknown/wasm-release/$lib.wasm"
    wasm-opt -Oz "${FEATURES[@]}" "$dest/pkg/${lib}_bg.wasm" -o "$dest/ship/opt.wasm"
    rm -rf "$dest/pkg"
  fi
  if [ "$named" = 1 ]; then
    (cd "$wl/app-$template" && CARGO_TARGET_DIR="$named_t" MF2_WORKLOAD_LOCALES="$wl" \
      CARGO_PROFILE_WASM_RELEASE_STRIP=false nice -n 10 cargo build --quiet "${args[@]}") || return 1
    mkdir -p "$dest/npkg"
    wasm-bindgen --target web --no-typescript --out-dir "$dest/npkg" \
      "$named_t/wasm32-unknown-unknown/wasm-release/$lib.wasm"
    wasm_opt_named "$dest/npkg/${lib}_bg.wasm" "$dest/named.wasm"
    rm -rf "$dest/npkg"
  fi
}

# A demo: $1 dest, $2 demo, then how it ships.
build_demo() {
  local dest="$1" demo="$2"
  local dir="$root/examples/$demo"
  if [ "$ship" = 1 ]; then
    mkdir -p "$dest/ship"
    case "$demo" in
      demo-ssr)
        rm -rf "$dir/target/site/pkg"   # no chunk of an earlier case counted
        (cd "$dir" && nice -n 10 cargo leptos build --release --split --frontend-only --cargo-offline \
          > "$dest/build.log" 2>&1) || return 1
        cp -r "$dir/target/site/pkg/." "$dest/ship/" ;;
      demo-islands)
        rm -rf "$dir/target/site/pkg"   # no chunk of an earlier case counted
        (cd "$dir" && nice -n 10 cargo leptos build --release --frontend-only --cargo-offline \
          > "$dest/build.log" 2>&1) || return 1
        cp -r "$dir/target/site/pkg/." "$dest/ship/" ;;
      demo-csr)
        (cd "$dir" && nice -n 10 trunk build --release > "$dest/build.log" 2>&1) || return 1
        find "$dir/dist" -maxdepth 1 \( -name '*.wasm' -o -name '*.js' \) -exec cp {} "$dest/ship/" \; ;;
    esac
    find "$dest/ship" -type f ! \( -name '*.wasm' -o -name '*.js' \) -delete
    node "$here/../p10-names/measure-demo.mjs" "$dest/ship" "$client/$case" > "$dest/node-measure.md"
  fi
  if [ "$named" = 1 ]; then
    local t="$a9/t-$demo-named" wasm
    if [ "$demo" = demo-csr ]; then
      (cd "$dir" && CARGO_TARGET_DIR="$t" CARGO_PROFILE_RELEASE_STRIP=false nice -n 10 cargo build --quiet \
        --release --target wasm32-unknown-unknown --offline) || return 1
      wasm="$t/wasm32-unknown-unknown/release/demo-csr.wasm"
    else
      (cd "$dir" && CARGO_TARGET_DIR="$t" CARGO_PROFILE_WASM_RELEASE_STRIP=false nice -n 10 cargo build --quiet \
        --lib --target wasm32-unknown-unknown --profile wasm-release --no-default-features --features hydrate --offline) || return 1
      wasm="$t/wasm32-unknown-unknown/wasm-release/${demo//-/_}.wasm"
    fi
    mkdir -p "$dest/npkg"
    wasm-bindgen --target web --no-typescript --out-dir "$dest/npkg" "$wasm"
    wasm_opt_named "$(ls "$dest"/npkg/*_bg.wasm)" "$dest/named.wasm"
    rm -rf "$dest/npkg"
  fi
}

# Only type-check a demo's client (A9_CHECK=1): does the case compile, and
# with which errors. The output goes to $dest/check.log.
check_demo() {
  local dest="$1" demo="$2"
  local dir="$root/examples/$demo" t="$a9/t-$demo-check"
  [ "$demo" = demo-ssr-08 ] && dir="$root/target/a7-demo-0-8/a9/examples/demo-ssr"
  if [ "$demo" = demo-csr ]; then
    (cd "$dir" && CARGO_TARGET_DIR="$t" nice -n 10 cargo check --target wasm32-unknown-unknown --offline \
      > "$dest/check.log" 2>&1)
  else
    (cd "$dir" && CARGO_TARGET_DIR="$t" nice -n 10 cargo check --lib --target wasm32-unknown-unknown \
      --no-default-features --features hydrate --offline > "$dest/check.log" 2>&1)
  fi
}

for case in "$@"; do
  dest="$a9/out/$lib/$client/$case"
  [ "${A9_CHECK:-0}" = 1 ] && dest="$a9/check/$lib/$client/$case"
  # Each kind of run replaces only its own outputs: a names-kept run keeps
  # the shipped files an earlier run left, and the other way round.
  if [ "${A9_CHECK:-0}" = 1 ]; then
    rm -rf "$dest"
  else
    [ "$ship" = 1 ] && rm -rf "$dest/ship" "$dest/raw.wasm" "$dest/build.log" "$dest/node-measure.md"
    [ "$named" = 1 ] && rm -f "$dest/named.wasm"
  fi
  mkdir -p "$dest"
  python3 "$here/case.py" apply "$client" "$case"
  log "start $lib $client $case (load $(cut -d' ' -f1 /proc/loadavg))"
  t0=$(date +%s)
  status=0
  if [ "${A9_CHECK:-0}" = 1 ]; then
    check_demo "$dest" "$client" || status=$?
    python3 "$here/case.py" restore "$client"
    log "check $lib $client $case: exit $status in $(( $(date +%s) - t0 )) s ($(grep -c '^error' "$dest/check.log" || true) errors)"
    continue
  fi
  case "$client" in
    fixture) build_fixture "$dest" || status=$? ;;
    tr) build_workload "$dest" "$root/target/size/wl-1860" tr "$root/target/a5/named/target-size-wl-1860" || status=$? ;;
    tr-view) build_workload "$dest" "$root/target/b5/wl-view-1860" tr-view "$a9/t-trview-named" || status=$? ;;
    demo-*) build_demo "$dest" "$client" || status=$? ;;
    *) echo "unknown client $client" >&2; status=2 ;;
  esac
  python3 "$here/case.py" restore "$client"
  t1=$(date +%s)
  if [ "$status" != 0 ]; then
    log "FAILED $lib $client $case (exit $status, $((t1 - t0)) s)"
    continue
  fi
  log "built $lib $client $case in $((t1 - t0)) s"
  if [ "$ship" = 1 ]; then
    python3 "$here/measure.py" "$lib" "$client" "$case" "$dest/ship" | tail -1
  fi
done
