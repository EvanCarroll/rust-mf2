#!/usr/bin/env bash
# P0.7, all measurements in one command (from any directory):
#   corpus stats -> out/stats.md; every layout variant × locale, each decoded
#   and compared with the parsed model -> out/sizes.tsv + out/tables.md;
#   the recommended catalogs (stripped + unstripped) -> out/catalogs/.
# Input: corpus/json/<tag>.json, produced by
#   cargo xtask gen-workload locales --out probes/p0-07-catalog-encoding/corpus
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BUILD_JOBS=3 cargo build -q --release
CARGO_BUILD_JOBS=3 cargo test -q --release
./target/release/p07 stats > out/stats.md
./target/release/p07 measure --out out > out/measure.log
./target/release/p07 emit --out out/catalogs >> out/measure.log
cat out/stats.md out/tables.md
