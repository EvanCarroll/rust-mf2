#!/usr/bin/env bash
# P0.9: wall-clock of compiling the 2,000-site app with
#   tr      — the design (generated macro_rules wrapper → proc-macro, manifest from the baked path)
#   trivial — same wrapper path and syntax, a proc-macro that reads no manifest and checks nothing
#   direct  — no macro: the expansion written out
# Only the app crate is recompiled (touch src/lib.rs; CARGO_INCREMENTAL=0 so each
# run is a full front-end pass); runs are interleaved tr/trivial/direct so load
# drift hits all three alike. Prints every run and the median per variant.
#
# Usage: scripts/time-expansions.sh MODE RUNS
#   MODE: check-ssr   cargo check  --lib --features ssr                      (native)
#         build-ssr   cargo build  --lib --features ssr                      (native, debug)
#         check-wasm  cargo check  --lib --features hydrate --target wasm32-unknown-unknown
# Env: P09_MANIFEST_MODE=inline to time the inline-manifest fallback (build.rs reruns once).
#      VARIANTS="tr trivial direct" to choose variants.
set -uo pipefail
cd "$(dirname "$0")/.."
mode="${1:?mode}"; runs="${2:-5}"
variants="${VARIANTS:-tr trivial direct}"
case "$mode" in
    check-ssr)  cmd=(check --lib --no-default-features --features ssr) ;;
    build-ssr)  cmd=(build --lib --no-default-features --features ssr) ;;
    check-wasm) cmd=(check --lib --no-default-features --features hydrate --target wasm32-unknown-unknown) ;;
    *) echo "bad mode" >&2; exit 2 ;;
esac
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=3
stats="$PWD/target/p09-macro-stats"
rm -f "$stats".*
echo "# warm-up (dependencies, i18n crate)"
for v in $variants; do
    cargo "${cmd[@]}" -p "workload-app-$v" >/dev/null 2>&1 || { echo "warm-up failed for $v"; cargo "${cmd[@]}" -p "workload-app-$v" 2>&1 | tail -20; exit 1; }
done
declare -A all
for r in $(seq 1 "$runs"); do
    for v in $variants; do
        touch "apps/$v/src/lib.rs"
        t0=$(date +%s.%N)
        if [ "$v" = tr ]; then
            P09_MACRO_STATS="$stats" cargo "${cmd[@]}" -p "workload-app-$v" >/dev/null 2>&1 || echo "FAILED $v"
        else
            cargo "${cmd[@]}" -p "workload-app-$v" >/dev/null 2>&1 || echo "FAILED $v"
        fi
        t1=$(date +%s.%N)
        dt=$(echo "$t1 - $t0" | bc)
        all[$v]="${all[$v]:-} $dt"
        printf 'run %d %-8s %6.2fs\n' "$r" "$v" "$dt"
    done
done
echo "# medians ($mode, $runs runs, P09_MANIFEST_MODE=${P09_MANIFEST_MODE:-path})"
for v in $variants; do
    med=$(tr ' ' '\n' <<<"${all[$v]}" | grep . | sort -n | awk '{a[NR]=$1} END{print (NR%2)?a[(NR+1)/2]:(a[NR/2]+a[NR/2+1])/2}')
    printf '%-8s median %6.2fs   runs:%s\n' "$v" "$med" "${all[$v]}"
done
echo "# proc-macro self-timing (tr, one line per rustc process):"
cat "$stats".* 2>/dev/null | awk '{printf "  expansions %s  in-macro %.1f ms  manifest reads %s\n", $4, $6/1e6, $8}'
