#!/bin/sh
# A6: what an edit to the translations does to the two builds, under
# `cargo leptos build` (as `cargo xtask scenarios` asks of the fixture): the
# wasm's and the server's hashes after each step, and whether the manifest
# hash moved. Each edit is reverted before the next.
#
#   ./scenario.sh [--split]
set -eu
cd "$(dirname "$0")/hello"
SPLIT=${1:-}
build() {
    started=$(date +%s)
    CARGO_BUILD_JOBS=1 cargo leptos build $SPLIT >../results/scenario-build.log 2>&1 ||
        { tail -30 ../results/scenario-build.log; exit 1; }
    echo "  build: $(($(date +%s) - started)) s"
}
snap() {
    wasm=$(sha256sum target/site/pkg/hello.wasm | cut -c1-16)
    server=$(sha256sum target/debug/hello | cut -c1-16)
    hash=$(cat target/debug/build/hello-*/out/mf2_generated_3c.rs target/front/wasm32-unknown-unknown/debug/build/hello-*/out/mf2_generated_3c.rs 2>/dev/null | grep -h -o 'MANIFEST_HASH: u64 = 0x[0-9a-f_]*' | sort -u | tr '\n' ' ')
    echo "  wasm $wasm  server $server  $hash"
}
edit() { # file from to
    grep -q -F -- "$2" "$1" || { echo "no match: $2"; exit 1; }
    python3 -c 'import sys; p,a,b=sys.argv[1:]; s=open(p).read(); open(p,"w").write(s.replace(a,b,1))' "$1" "$2" "$3"
}
step() { # name
    echo "$1"
    build
    snap
}

step "S0 baseline (after the first build)"
step "S1 nothing changed"
touch locales/fr/main.mf2
step "S2 mtime only (fr touched)"
edit locales/fr/main.mf2 "visit-again = Revenir" "visit-again = Revenez"
step "S3 translation only (fr: visit-again)"
edit locales/fr/main.mf2 "visit-again = Revenez" "visit-again = Revenir"
step "S3' reverted"
edit locales/en/main.mf2 "visit-again = Visit again" "visit-again = Come again"
step "S4 source text (en: visit-again)"
edit locales/en/main.mf2 "visit-again = Come again" "visit-again = Visit again"
step "S4' reverted"
