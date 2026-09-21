#!/usr/bin/env bash
# P0.1, the whole measurement in one command (resumable; each app build is
# skipped if its row is already in target/results.tsv):
#
#   probes/p0-01-call-site/scripts/run-all.sh
#
# 1. variant templates (data only) -> target/templates/
# 2. the default workload (M=1860, N=1600, K=60) and the 2x workload
#    (M=3720, N=3200, K=120) with every template -> target/wl-1860, wl-3720
# 3. every app: wasm-release -> wasm-bindgen -> wasm-opt -Oz -> gzip -9,
#    one at a time, CARGO_BUILD_JOBS=3, one target dir per workload
# 4. tables -> target/tables.md
set -euo pipefail
probe=$(realpath "$(dirname "$0")/..")
root=$(realpath "$probe/../..")
rel=${probe#"$root"/}
cd "$root"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
cargo xtask gen-workload templates --dump closure --out "$rel/target/closure-builtin" >/dev/null
rm -rf "$probe/target/templates"
python3 "$probe/scripts/variants.py" "$probe/target/templates" "$probe/target/closure-builtin" >/dev/null
T=(-t "$rel/templates/idlit" -t "$rel/templates/dummy" -t literal -t closure -t "$rel/templates/tr")
for v in $(ls "$probe/target/templates"); do T+=(-t "$rel/target/templates/$v"); done
# Regenerating rewrites sources (new mtimes => rebuild); skip if present.
[ -f "$probe/target/wl-1860/sites.json" ] || cargo xtask gen-workload all "${T[@]}" --out "$rel/target/wl-1860"
[ -f "$probe/target/wl-3720/sites.json" ] || cargo xtask gen-workload all "${T[@]}" -m 3720 -n 3200 -k 120 --out "$rel/target/wl-3720"
apps=()
for wl in wl-1860 wl-3720; do
  for t in idlit dummy literal closure tr $(ls "$probe/target/templates"); do apps+=("$probe/target/$wl/app-$t"); done
done
"$probe/scripts/queue.sh" "$probe/target/results.tsv" "${apps[@]}"
python3 "$probe/scripts/analyze.py" > "$probe/target/tables.md"
echo "tables: $rel/target/tables.md"
