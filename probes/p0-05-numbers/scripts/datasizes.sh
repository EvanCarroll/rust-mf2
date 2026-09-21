#!/usr/bin/env bash
# Per-locale LOCALE entry sizes (raw / standalone gzip -9) from out/locale.
set -euo pipefail
cd "$(dirname "$0")/.."
printf '%-6s' loc
for g in core core+percent cur-patterns cur-used cur-all unit-used unit-all; do printf ' %16s' "$g"; done; echo
for loc in en es de fr ar he ja hi ru pl cy; do
  printf '%-6s' $loc
  for g in core core+percent cur-patterns cur-used cur-all unit-used unit-all; do
    if [ "$g" = core+percent ]; then cat out/locale/$loc.core.bin out/locale/$loc.percent.bin > out/locale/.tmp; f=out/locale/.tmp; else f=out/locale/$loc.$g.bin; fi
    printf ' %8s' "$(stat -c %s $f)/$(gzip -9 -c $f | wc -c)"
    printf '%8s' ''
  done; echo
done
rm -f out/locale/.tmp
