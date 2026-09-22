#!/usr/bin/env bash
# The data every item reads, into target/intl-probe/data/ (generated, not
# committed): the timed catalogs, the L4 bundles, the CLDR plural samples,
# P0.5's 100,000 ECMA-402 cases (runtime-bench, root workspace) and their
# catalogs, the edge cases, the locale-symbol cases and their Rust output.
#
#   bench/intl-probe/scripts/data.sh                       # working tree
#   INTL_PROBE_REV=3fc4735 bench/intl-probe/scripts/data.sh
source "$(dirname "$0")/common.sh"
D="$OUT/data"
mkdir -p "$D"
# With mf2-fn-number (Phase 4 A3 on): the Rust side of the locale-symbol
# cases is localized, and the catalog-data table is written.
features=()
if grep -q 'pub static PERCENT' "$SRC/crates/mf2-fn-number/src/lib.rs" 2>/dev/null; then
  features=(--features fn-number,number-data)
fi
(cd "$PROBE" && cargo build -q --release -p intl-probe-native "${features[@]}")
N="$PROBE/target/release/intl-probe-native"
for c in speed l4 plural edge loc loc-format; do
  "$N" --repo "$REPO" --out "$D" "$c"
done
# P0.5's corpus through today's Rust path (bench/runtime-bench, A5).
(cd "$SRC" && cargo run -q --release -p runtime-bench -- numbers ecma 100000) > "$D/ecma.jsonl"
"$N" --repo "$REPO" --out "$D" ecma --input "$D/ecma.jsonl"
"$N" observe > "$D/observe.tsv"
[ ${#features[@]} -gt 0 ] && "$N" --repo "$REPO" --out "$D" catalog-data
echo "data: $(tree_note)" | tee "$D/tree.txt"
