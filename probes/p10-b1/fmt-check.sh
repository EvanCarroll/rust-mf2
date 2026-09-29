#!/usr/bin/env bash
# Phase 10 B1: A9's `fmt-check.sh` (probes/p10-display-cost/) for the merged
# crate — does a client build link a `Display` or `Debug` impl of one of our
# types, the blanket `ToString` over one, or the `Debug` writers?
#
#   fmt-check.sh WASM...     (names kept: a debug-profile client build)
#
# The patterns are A9's, with the crates B1 moved the types into (`mf2`,
# `mf2_leptos_ui_0_9` / `_0_8`) and `mf2::debug::`, the write_str `Debug`
# helpers. Not `mf2::display::ambient`: the inherent `to_string()` shares it.
set -uo pipefail
H='(\[[0-9a-f]+\])?'
OURS="(leptos_mf2|mf2_leptos_ui_0_[89]|mf2_runtime|mf2_model|mf2_catalog|mf2_fn_[a-z]+|mf2_host_[a-z]+|mf2)$H::"
IMPL="<[^ ]*$OURS[^ ]* as core$H::fmt::(Display|Debug)>::fmt|<[^ ]*$OURS[^ ]* as alloc$H::string::(ToString|SpecToString)>|mf2$H::debug::"
status=0
for wasm in "$@"; do
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
