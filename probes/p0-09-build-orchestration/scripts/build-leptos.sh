#!/usr/bin/env bash
# P0.9: `cargo leptos build` (server + front, as cargo-leptos runs them) with
# the rebuilt workspace crates, the exit status and the first error.
# Usage: scripts/build-leptos.sh [extra cargo-leptos args]
set -uo pipefail
cd "$(dirname "$0")/.."
log=$(mktemp)
t0=$(date +%s.%N)
CARGO_BUILD_JOBS=2 cargo leptos build "$@" >"$log" 2>&1
rc=$?
t1=$(date +%s.%N)
printf '[cargo leptos build %s] exit=%s  %.1fs\n' "$*" "$rc" "$(echo "$t1 - $t0" | bc)"
sed 's/\x1b\[[0-9;]*m//g' "$log" | grep -E '^\s+Compiling (p09|workload)' | sed -E 's/^\s+Compiling ([^ ]+).*/  compiled: \1/' | sort | uniq -c | sed 's/^ */  /'
sed 's/\x1b\[[0-9;]*m//g' "$log" | grep -E 'Finished generating|Cargo finished|Front|Serving|Error|error:' | sed 's/^/  /' | cut -c1-200 | head -12
if [ $rc -ne 0 ]; then
    sed 's/\x1b\[[0-9;]*m//g' "$log" | awk '/^error/{p=1} p{print "  | " $0} /^$/{if(p) exit}' | head -30
fi
rm -f "$log"
