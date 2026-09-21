#!/usr/bin/env bash
# P0.9: the two builds cargo-leptos runs, done with plain cargo, one after
# the other (server: native, ssr; client: wasm32, hydrate). Prints, per build:
# exit status, wall-clock, which workspace units were dirty (and why), which
# build scripts ran, and the first compile error (if any).
# Usage: scripts/build-plain.sh [ssr|wasm|both]
set -uo pipefail
cd "$(dirname "$0")/.."
which="${1:-both}"
run() {
    local name=$1; shift
    local log; log=$(mktemp)
    local t0; t0=$(date +%s.%N)
    CARGO_BUILD_JOBS=3 cargo build -v "$@" >"$log" 2>&1
    local rc=$?
    local t1; t1=$(date +%s.%N)
    printf '[%s] exit=%s  %.1fs\n' "$name" "$rc" "$(echo "$t1 - $t0" | bc)"
    grep -E '^\s+Dirty (p09|workload)' "$log" | sed -E 's/^\s+/  /; s#/home/[^ )]*/probes/p0-09-build-orchestration/##g' | cut -c1-220
    grep -E '^\s+Compiling (p09|workload)' "$log" | sed -E 's/^\s+Compiling ([^ ]+).*/  compiled: \1/' | sort | uniq -c | sed 's/^ */  /'
    grep -E 'Running `.*p09-i18n-[0-9a-f]+/build-script-build`' "$log" | sed -E 's/.*(build\/p09-i18n-[0-9a-f]+).*/  build.rs ran: \1/'
    if [ $rc -ne 0 ]; then
        awk '/^error/{p=1} p{print "  | " $0} /^$/{if(p) exit}' "$log" | head -40
        grep -c '^error' "$log" | sed 's/^/  errors: /'
    fi
    rm -f "$log"
}
[ "$which" != wasm ] && run ssr --package=workload-app-tr --bin=workload-app-tr --no-default-features --features=ssr
[ "$which" != ssr ] && run wasm --package=workload-app-tr --lib --target=wasm32-unknown-unknown --no-default-features --features=hydrate
exit 0
