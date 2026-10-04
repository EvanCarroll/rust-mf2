#!/usr/bin/env bash
# The number split (plan/08 §6, task 18.4): `:currency` and `:unit` with
# their names from Intl.NumberFormat and their digits, rounding and plural
# selection in Rust (variant `rt-names-cu`), against the Rust path
# (`rust-cu`), on the probe's locale-symbol panel. Four measurements:
#
#   1. the client's gzip bytes: wasm + JS glue of each variant over `base`
#      (build.sh; `rt-intl-cu`, the whole `intl` option, for reference);
#   2. a catalog's brotli bytes per language, with and without
#      `currency.data` and `unit.data` (intl-probe-native names-data);
#   3. the time per placeholder in Chromium, Firefox and WebKit, the two
#      variants alternated in one page (the `names` item of the Playwright
#      check `intl`);
#   4. where the text differs from the Rust registry (the same item).
#
# Writes bench/intl-probe/NAMES-RESULTS.md, with this command and the tree.
#
#   bench/intl-probe/scripts/7-names.sh
#   INTL_PROBE_REV=<rev> bench/intl-probe/scripts/7-names.sh   # a commit's crates
#
# Tools: those of build.sh; node and the Playwright browsers (`npm install`
# in tools/e2e).
source "$(dirname "$0")/common.sh"
here="$REPO/bench/intl-probe/scripts"
command -v node >/dev/null 2>&1 || { echo "7-names.sh: node not found" >&2; exit 2; }
started=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# 1. The modules.
PROBE_VARIANTS="base rust-cu rt-names-cu rt-intl-cu" "$here/build.sh" > "$OUT/names-build.log"

# 2. The panel's catalogs, the Rust registry's text, the catalog bytes.
D="$OUT/data"
mkdir -p "$D"
(cd "$PROBE" && cargo build -q --release -p intl-probe-native --features fn-number,number-data)
N="$PROBE/target/release/intl-probe-native"
for c in loc loc-format names-data; do
  "$N" --repo "$REPO" --out "$D" "$c" > "$OUT/names-$c.log"
done

# 3 and 4. The three engines.
mkdir -p "$OUT/results"
status=0
(cd "$REPO/tools/e2e" && MF2_INTL_ITEMS=names node run.mjs intl --browser all \
  --label names --json "$OUT/results/e2e-names.json") || status=$?

machine="$(nproc) CPUs, $(mhz) MHz at the end, load $(cut -d' ' -f1-3 /proc/loadavg)"
node "$here/names-report.mjs" \
  --command "bench/intl-probe/scripts/7-names.sh${INTL_PROBE_REV:+ (INTL_PROBE_REV=$INTL_PROBE_REV)}" \
  --tree "$(tree_note)" --machine "$machine" --started "$started" \
  --out "$REPO/bench/intl-probe/NAMES-RESULTS.md"
echo "7-names.sh: bench/intl-probe/NAMES-RESULTS.md"
exit "$status"
