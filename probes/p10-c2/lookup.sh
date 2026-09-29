#!/usr/bin/env bash
# Phase 10 C2: the ambient lookup's first step, timed (A4 left it to C2):
# `lookup/` built with `native` alone and with `ssr` beside it, each binary
# kept, then RUNS runs of each, alternating, so that load and clock drift fall
# on both alike. Every line is one run's medians (ns per call).
#
#   bash probes/p10-c2/lookup.sh [RUNS] [ROUNDS]
#
# Outputs in target/p10-c2/lookup/; the build directory is removed at the end.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-c2/lookup
runs=${1:-21}
rounds=${2:-51}
mkdir -p "$out"
export CARGO_NET_OFFLINE=true
build() {
  CARGO_BUILD_JOBS=3 cargo build --release --offline --manifest-path probes/p10-c2/lookup/Cargo.toml \
    --target-dir "$out/t" "$@" >>"$out/build.log" 2>&1
}
build
cp "$out/t/release/lookup-bench" "$out/native"
build --features ssr
cp "$out/t/release/lookup-bench" "$out/ssr"
: >"$out/runs.jsonl"
echo "load $(cut -d' ' -f1-3 /proc/loadavg)" | tee "$out/load.txt"
for _ in $(seq 1 "$runs"); do
  "$out/native" "$rounds" >>"$out/runs.jsonl"
  "$out/ssr" "$rounds" >>"$out/runs.jsonl"
done
echo "load $(cut -d' ' -f1-3 /proc/loadavg)" | tee -a "$out/load.txt"
python3 - "$out/runs.jsonl" <<'PY' | tee "$out/report.md"
import json, statistics, sys
rows = {}
for line in open(sys.argv[1]):
    run = json.loads(line)
    for k, v in run.items():
        if k in ("build", "check"):
            continue
        rows.setdefault(k, {}).setdefault(run["build"], []).append(v)
print("| row | native, median ns (range) | ssr+native, median ns (range) | difference |")
print("|---|---:|---:|---:|")
for k, by in rows.items():
    a, b = by["native"], by["ssr+native"]
    ma, mb = statistics.median(a), statistics.median(b)
    print(f"| {k} | {ma:.2f} ({min(a):.2f}–{max(a):.2f}) | {mb:.2f} ({min(b):.2f}–{max(b):.2f}) | {mb - ma:+.2f} ({(mb / ma - 1) * 100:+.1f} %) |")
PY
rm -rf "$out/t"
