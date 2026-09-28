#!/usr/bin/env bash
# A9: the check a CI job could run — does a client build link a `Display` or
# `Debug` impl of one of our types (or our helpers behind them)?
#
#   fmt-check.sh WASM...     (names kept: a build with strip = false)
#
# Exit 1, listing them, if any WASM does; 0 if none does. The shipped wasm
# has no names, so the input is the same client built with
# CARGO_PROFILE_<profile>_STRIP=false (run.sh with A9_NAMED=1); twiggy
# demangles the v0 symbols, with crate hashes (`core[…]::fmt`). Generic
# impls instantiated over one of our types (`<&Tr as Display>`,
# `<Arc<TrArgs> as Display>`, a read guard's) name the type too, so they
# match.
set -uo pipefail
H='(\[[0-9a-f]+\])?'
OURS="(leptos_mf2|mf2_runtime|mf2_model|mf2_catalog|mf2_fn_[a-z]+|mf2_host_[a-z]+|mf2)$H::"
# An impl of ours, or a generic one over our types; the blanket
# `ToString` over them (a wrapper's `.to_string()` that took `Display`, in
# case LLVM inlined `fmt` into it); the helpers only `Display` / `Debug`
# reach. Not `display::ambient::string`: the inherent `to_string()` uses it.
IMPL="<[^ ]*$OURS[^ ]* as core$H::fmt::(Display|Debug)>::fmt|<[^ ]*$OURS[^ ]* as alloc$H::string::(ToString|SpecToString)>|leptos_mf2$H::display::ambient::fmt|leptos_mf2$H::debug::|leptos_mf2$H::text::fmt_display"
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
