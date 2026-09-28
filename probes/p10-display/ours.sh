#!/usr/bin/env bash
# A5: one named build's fmt picture, lines cut to 240 columns.
#   ours.sh DIR   (DIR holds opt.wasm)
set -uo pipefail
d="$1"
FMT_RE='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|4core3fmt|5alloc3fmt|fmt::Formatter|fmt::Arguments|Debug|Display'
IMPL_RE='as core(\[[0-9a-f]+\])?::fmt::(Display|Debug)>::fmt'
[ -f "$d/top.txt" ] || twiggy top -n 1000000 "$d/opt.wasm" > "$d/top.txt" 2>/dev/null
echo "== $d"
echo "  functions matching the B12 fmt pattern: $(grep -cE "$FMT_RE" "$d/top.txt") ($(grep -E "$FMT_RE" "$d/top.txt" | awk '{s+=$1} END {print s+0}') B)"
echo "  Display/Debug impls of mf2 types (any crate of ours):"
grep -E "$IMPL_RE" "$d/top.txt" | grep -E "<[^ ]*(leptos_mf2|mf2_[a-z_]+|mf2)\[" | cut -c1-240 | sed 's/^/    /'
echo "  every Display/Debug impl in the module (type names only):"
grep -E "$IMPL_RE" "$d/top.txt" | sed -E 's/^ *([0-9]+).*┊ <(.*) as core.*::fmt::(Display|Debug)>::fmt.*/\1 \3 \2/' | cut -c1-160 | sed 's/^/    /'
