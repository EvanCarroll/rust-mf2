#!/usr/bin/env bash
# Everything RESULT.md reports, in order (from probes/p0-05-numbers/).
set -euo pipefail
cd "$(dirname "$0")/.."
T=../../third_party/message-format-wg/test/tests/functions
mkdir -p out
CARGO_BUILD_JOBS=3 cargo test -q -p numcore
# 1. core semantics: suite (neutral) + ECMA-402 differential (100k cases)
CARGO_BUILD_JOBS=3 cargo run -q -p suite --bin suite -- $T/number.json $T/integer.json $T/offset.json | tee out/suite-core.txt | tail -1
CARGO_BUILD_JOBS=3 cargo run -q --release -p suite --bin ecma_cases -- 100000 | node scripts/ecma-diff.cjs | tee out/ecma-diff.txt
# 2./3. locale data, then fn-number: suite (en-US data), icu_decimal differential, Intl comparison
CARGO_BUILD_JOBS=3 cargo run -q --release -p locdata -- --cldr ../../third_party/cldr-json --out out/locale
scripts/datasizes.sh | tee out/datasizes.txt
CARGO_BUILD_JOBS=3 cargo run -q -p suite --bin suite -- --loc out/locale/en.all-used.bin \
  $T/number.json $T/integer.json $T/offset.json $T/percent.json $T/currency.json | tee out/suite-loc.txt | tail -1
CARGO_BUILD_JOBS=3 cargo run -q --release -p diff --bin diff | tee out/diff-icu-decimal.txt | tail -14
CARGO_BUILD_JOBS=3 cargo run -q --release -p diff --bin loc_cases | node scripts/intl-loc.cjs | tee out/intl-loc.txt | tail -4
# sizes (06 §3 method) and B12
scripts/measure.sh base core core-f64 loc loc-cu icu-decimal-blob intl-num | tee out/sizes.txt
scripts/b12.sh | tee out/b12.txt
