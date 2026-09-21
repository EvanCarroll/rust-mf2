#!/usr/bin/env bash
# P0.4 data sizes: our encoded plural.* entries for every locale (p04 sizes)
# and, for context, gzip -9 of the panel's entries next to ICU4X 2.3 payloads
# and BlobDataProvider blobs exported from the same CLDR 48.2.1 files.
# (gzip adds ~20 B of header/trailer, so gz > raw for inputs this small.)
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO_BUILD_JOBS=3 cargo build -q --release -p plural-rules -p icu-data
target/release/p04 sizes --out out/data
echo
target/release/icu-data sizes --out out/icu
echo
gz() { gzip -9 -n -c "$1" | wc -c; }
printf '%-7s %14s %14s %14s | %18s %18s\n' locale "ours card" "ours ord" "ours both" "ICU blob card" "ICU blob card+ord"
printf '%-7s %14s %14s %14s | %18s %18s\n' "" "raw/gz" "raw/gz" "raw/gz" "raw/gz" "raw/gz"
rg_() { printf '%s/%s' "$(stat -c %s "$1")" "$(gz "$1")"; }
for l in en es de fr ar he ja hi ru pl cy; do
  printf '%-7s %14s %14s %14s | %18s %18s\n' "$l" \
    "$(rg_ out/data/$l.cardinal.bin)" "$(rg_ out/data/$l.ordinal.bin)" "$(rg_ out/data/$l.both.bin)" \
    "$(rg_ out/icu/$l.icu-card.blob)" "$(rg_ out/icu/$l.icu-card-ord.blob)"
done
f=out/icu/en-ar-ru.icu-card.blob
echo "ICU blob en+ar+ru cardinal: $(stat -c %s $f)/$(gz $f) B raw/gz"
