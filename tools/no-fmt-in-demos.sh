#!/usr/bin/env bash
# Does a client build link a `Display` or `Debug` impl of one of our types,
# the blanket `ToString` over one, or the `Debug` writers? (A9's check, as B1
# moved it to the merged crate.)
#
#   tools/no-fmt-in-demos.sh WASM...   (names kept: a debug-profile client build)
#
# A release build, even with names kept, misses the wrapper traps (A9), so
# the input is a debug build. twiggy demangles the v0 symbols, with crate
# hashes (`core[…]::fmt`); a generic impl over one of our types (`<&Tr as
# Display>`) names the type too, so it matches. Not `mf2::display::ambient`:
# the inherent `to_string()` shares it. Exit 1, listing them, if any WASM
# links one; 0 if none does.
set -uo pipefail
H='(\[[0-9a-f]+\])?'
OURS="(mf2_leptos_ui_0_[89]|mf2_runtime|mf2_model|mf2_catalog|mf2_fn_[a-z]+|mf2_host_[a-z]+|mf2)$H::"
IMPL="<[^ ]*$OURS[^ ]* as core$H::fmt::(Display|Debug)>::fmt|<[^ ]*$OURS[^ ]* as alloc$H::string::(ToString|SpecToString)>|mf2$H::debug::"
status=0
for wasm in "$@"; do
  if [ ! -f "$wasm" ]; then
    echo "FAIL $wasm: no such file"
    status=1
    continue
  fi
  hits=$(twiggy top -n 1000000 "$wasm" 2>/dev/null | grep -E "$IMPL" | sed -E 's/^ *[0-9]+ ┊ *[0-9.]+% ┊ //' | sort -u)
  if [ -n "$hits" ]; then
    echo "FAIL $wasm: $(printf '%s\n' "$hits" | wc -l) of our Display / Debug items"
    printf '%s\n' "$hits" | cut -c1-200 | sed 's/^/  /'
    status=1
  else
    echo "ok   $wasm"
  fi
done
exit $status
