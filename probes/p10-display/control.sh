#!/usr/bin/env bash
# A5 positive control: the fixture client with one `format!("{save} {items:?}")`
# (a working-tree edit, never committed), built with names like named.sh.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
export CARGO_BUILD_JOBS=2
target="$root/target/a5/named/target-fixture"
dest="$root/target/a5/named/control/fixture-hydrate+fn-number"
CARGO_TARGET_DIR="$target" CARGO_PROFILE_WASM_RELEASE_STRIP=false \
  nice -n 10 cargo build --quiet -p mf2-i18n-fixture --bin mf2-i18n-client --no-default-features \
    --features hydrate,fn-number --target wasm32-unknown-unknown --profile wasm-release
rm -rf "$dest"
mkdir -p "$dest"
cp "$target/wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm" "$dest/raw.wasm"
wasm-opt -Oz --debuginfo --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue "$dest/raw.wasm" -o "$dest/opt.wasm"
echo "control built: $dest"
