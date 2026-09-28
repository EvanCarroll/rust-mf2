#!/usr/bin/env bash
# A9: is `wasm-opt -Oz` of a names-kept build (strip = false) byte-identical
# to `wasm-opt -Oz` of the shipped build (strip = true)? If so, one build per
# case gives both the shipped bytes and twiggy's names.
source "$(dirname "$0")/lib.sh"
out="$a9/eq"
mkdir -p "$out"
cd "$root"
build() {
  local strip="$1" target="$2"
  CARGO_TARGET_DIR="$target" CARGO_PROFILE_WASM_RELEASE_STRIP="$strip" nice -n 10 cargo build --quiet \
    -p mf2-i18n-fixture --bin mf2-i18n-client --no-default-features --features hydrate,fn-number \
    --target wasm32-unknown-unknown --profile wasm-release
  cp "$target/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" "$out/strip-$strip.raw.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$out/strip-$strip.raw.wasm" -o "$out/strip-$strip.opt.wasm"
}
t0=$(date +%s); build false "$root/target/a5/named/target-fixture"; t1=$(date +%s)
build true "$a9/t-fixture-stripped"; t2=$(date +%s)
echo "named build $((t1 - t0)) s, stripped build $((t2 - t1)) s"
ls -l "$out"
cmp "$out/strip-false.opt.wasm" "$out/strip-true.opt.wasm" && echo "opt: byte-identical"
