#!/usr/bin/env bash
# P0.9 A/B: debug build (cargo build --lib --features ssr, CARGO_INCREMENTAL=0) of
# the 2,000-site tr app with the expansion's tokens spanned at the user's id
# literal (default) vs. the proc-macro call site (P09_CALLSITE_SPANS=1), with
# the hand-expanded `direct` app as control; interleaved. (The A/B switch
# P09_CALLSITE_SPANS was removed from the macro after this measurement.)
# Usage: scripts/time-spans.sh RUNS
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=3
cmd=(build --lib --no-default-features --features ssr)
cargo "${cmd[@]}" -p workload-app-tr >/dev/null 2>&1; cargo "${cmd[@]}" -p workload-app-direct >/dev/null 2>&1
for r in $(seq 1 "${1:-4}"); do
    for v in user callsite direct; do
        case $v in
            user) app=tr; env=() ;;
            callsite) app=tr; env=(P09_CALLSITE_SPANS=1) ;;
            direct) app=direct; env=() ;;
        esac
        touch "apps/$app/src/lib.rs"
        t0=$(date +%s.%N)
        env "${env[@]}" cargo "${cmd[@]}" -p "workload-app-$app" >/dev/null 2>&1 || echo "FAILED $v"
        printf 'run %d %-9s %6.1fs  rlib %s B\n' "$r" "$v" "$(echo "$(date +%s.%N) - $t0" | bc)" \
            "$(stat -c %s target/debug/libworkload_app_$app.rlib 2>/dev/null)"
    done
done
