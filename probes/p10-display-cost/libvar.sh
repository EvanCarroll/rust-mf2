#!/usr/bin/env bash
# A9: put a library variant of `crates/leptos-mf2` into the tree, or take it
# out again (back to the probe branch's A5 variant).
#
#   libvar.sh apply NAME      NAME: v1x s1-writestr s2-debug r1 r1d r2 r2d r3
#   libvar.sh restore
#
#   v1x          the crate as at 2fb7f54 (1.x's `Display`-less descriptions)
#   s1-writestr  `Display` through `write_str` in place of `Formatter::pad`
#   s2-debug     every `Debug` through `write_str`: no `core::fmt` number,
#                float or escape code, no builder
#   r1 / r1d     `Display` (r1d: and A5's `Debug`) only with the std mode
#   r2 / r2d     `Display` (r2d: and A5's `Debug`) not on wasm32-unknown-unknown
#   r3           on wasm32-unknown-unknown, `Display` refuses at compile time
#                with a message of ours (a bound on a never-implemented trait)
source "$(dirname "$0")/lib.sh"
cd "$root"
case "$1" in
  apply)
    if ! git diff --quiet -- crates/leptos-mf2 || [ -n "$(git ls-files --others --exclude-standard crates/leptos-mf2)" ]; then
      echo "crates/leptos-mf2 is not the probe branch's: restore first" >&2
      exit 1
    fi
    git apply "$here/lib-$2.patch"
    echo "$2" > "$a9/libvar"
    log "library variant $2 applied" ;;
  restore)
    git checkout -- crates/leptos-mf2
    git clean -fq -- crates/leptos-mf2
    rm -f "$a9/libvar"
    log "library variant restored to a5" ;;
  *) echo "usage: libvar.sh apply NAME | restore" >&2; exit 2 ;;
esac
