#!/usr/bin/env bash
# P0.9: drive `cargo leptos watch --split` non-interactively.
#   1. start it (log in target/p09-watch.log), wait until it serves;
#   2. append a comment to apps/tr/src/lib.rs (control: an app-source edit);
#   3. scripts/edit.sh text (a locale edit);
#   after each edit wait up to WAIT seconds for a new server build and report
#   whether /second shows the edited text; then stop everything and revert.
# Usage: scripts/watch-check.sh            (WAIT=150 by default)
set -uo pipefail
cd "$(dirname "$0")/.."
log=target/p09-watch.log
wait_s="${WAIT:-150}"
plain() { sed 's/\x1b\[[0-9;]*m//g' "$log"; }
builds() { sed 's/\x1b\[[0-9;]*m//g' "$log" | grep -c 'Cargo finished cargo build --package=workload-app-tr --bin'; }
cleanup() {
    kill -INT "$pid" 2>/dev/null; sleep 2; kill "$pid" 2>/dev/null
    for p in $(pgrep -f '^/[^ ]*/target/debug/workload-app-tr$'); do kill "$p" 2>/dev/null; done
    cp apps/tr/src/lib.rs.p09bak apps/tr/src/lib.rs 2>/dev/null && rm -f apps/tr/src/lib.rs.p09bak
    scripts/edit.sh revert >/dev/null
}
CARGO_BUILD_JOBS=2 cargo leptos watch --split >"$log" 2>&1 &
pid=$!
trap cleanup EXIT
for _ in $(seq 1 900); do plain | grep -q 'Serving at' && break; sleep 1; done
plain | grep -q 'Serving at' || { echo "watch did not start"; tail -20 "$log"; exit 1; }
echo "watch serving; server builds so far: $(builds)"
step() {
    local name=$1 expect=$2; shift 2
    local b0; b0=$(builds)
    "$@" >/dev/null
    local t0; t0=$(date +%s)
    local seen=no
    while [ $(( $(date +%s) - t0 )) -lt "$wait_s" ]; do
        if [ "$(builds)" -gt "$b0" ] && curl -sf -o /dev/null http://127.0.0.1:3709/manifest; then
            seen=yes; break
        fi
        sleep 1
    done
    local shown=n/a
    if [ -n "$expect" ]; then
        # the server restarts after its build finishes: poll until it answers with the edit
        for _ in $(seq 1 60); do
            shown=$(curl -s http://127.0.0.1:3709/second | grep -c -- "$expect")
            [ "$shown" -gt 0 ] && break
            sleep 1
        done
    fi
    printf '[%s] server rebuilt: %s after %ss; /second shows %q: %s\n' "$name" "$seen" "$(( $(date +%s) - t0 ))" "$expect" "$shown"
}
cp apps/tr/src/lib.rs apps/tr/src/lib.rs.p09bak
step app-source-edit "" sh -c 'echo "// p09 watch probe" >> apps/tr/src/lib.rs'
step locale-edit "EDITED-P09" scripts/edit.sh text
echo "rebuild lines in the log:"
sed 's/\x1b\[[0-9;]*m//g' "$log" | grep -E 'Compiling (p09|workload)|Serving at|Watching|changed|Error' | uniq -c | sed 's/^/  /' | head -20
