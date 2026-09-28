#!/usr/bin/env bash
# A5 positive control, shipped form: the fixture client built exactly as
# `cargo xtask b12-generated` builds build A (hydrate,fn-number; wasm-release,
# stripped), raw .wasm bytes — with and without the control's one
# `format!("{save} {items:?}")`. Run once with the edit in place (label
# control) and once without (label plain).
set -euo pipefail
label="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
export CARGO_BUILD_JOBS=2
target="$root/target/a5/target-fixture-stripped"
nice -n 10 env CARGO_TARGET_DIR="$target" cargo build --quiet -p mf2-i18n-fixture --bin mf2-i18n-client \
  --no-default-features --features hydrate,fn-number --target wasm32-unknown-unknown --profile wasm-release
wasm="$target/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm"
mkdir -p "$root/target/a5/stripped"
cp "$wasm" "$root/target/a5/stripped/$label.wasm"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  "$root/target/a5/stripped/$label.wasm" -o "$root/target/a5/stripped/$label.opt.wasm"
echo "$label raw $(stat -c %s "$root/target/a5/stripped/$label.wasm") opt $(stat -c %s "$root/target/a5/stripped/$label.opt.wasm") opt-gz $(gzip -9 -n -c "$root/target/a5/stripped/$label.opt.wasm" | wc -c)"
