#!/usr/bin/env bash
# A5: what twiggy sees in the named builds of one label (target/a5/named/$1):
# every function of ours that is Display / Debug / fmt, and the fmt machinery
# overall, so base and variant can be compared.
set -uo pipefail
label="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
dir="$root/target/a5/named/$label"
FMT_RE='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|4core3fmt|5alloc3fmt|fmt::Formatter|fmt::Arguments|Debug|Display'
for d in "$dir"/*/; do
  wasm="$d/opt.wasm"
  [ -f "$wasm" ] || continue
  name="$(basename "$d")"
  twiggy top -n 1000000 "$wasm" > "$d/top.txt" 2>/dev/null
  twiggy garbage "$wasm" > "$d/garbage.txt" 2>/dev/null
  ours=$(grep -E 'leptos_mf2|mf2_runtime|mf2_catalog|mf2::' "$d/top.txt" | grep -E "$FMT_RE" || true)
  fmt_count=$(grep -cE "$FMT_RE" "$d/top.txt" || true)
  fmt_bytes=$(grep -E "$FMT_RE" "$d/top.txt" | awk '{s+=$1} END {print s+0}')
  echo "== $label / $name"
  echo "  functions matching the B12 fmt pattern: $fmt_count ($fmt_bytes B, names kept)"
  echo "  ours among them:"
  if [ -n "$ours" ]; then printf '%s\n' "$ours" | sed 's/^/    /'; else echo "    none"; fi
done
