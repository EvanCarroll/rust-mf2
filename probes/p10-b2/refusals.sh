#!/usr/bin/env bash
# Phase 10 B2: what a user sees when `native` meets a browser build — 19
# §3's refusal — on `mf2` itself and through the `mf2-native` shim, for the
# browser's target and natively; and the combination that must compile,
# `native` beside `ssr`. Each misuse should show `mf2`'s one sentence, not a
# dependency's errors first. Since the browser-only refusal (plans/18
# question 24) the refusal is for `wasm32` alone: natively, `native` beside
# `hydrate` (`mf2-host-hydrate`) compiles too. `cargo xtask refusals`
# checks both sides in `ci`.
#
#   bash probes/p10-b2/refusals.sh
#
# Outputs (every cargo log) in target/p10-b2/refusals/. One build at a time.
set -u
cd "$(dirname "$0")/../.."
out=target/p10-b2/refusals
mkdir -p "$out"
check() {
  local name=$1; shift
  CARGO_BUILD_JOBS=3 cargo check "$@" --color never >"$out/$name.log" 2>&1
  local rc=$?
  echo "== $name: cargo check $* → exit $rc"
  # Every error the user reads, counted: our sentence should be the only one.
  grep -E '^error' "$out/$name.log" | sort | uniq -c
}
check mf2-wasm-hydrate -p mf2 --features native,leptos,hydrate --target wasm32-unknown-unknown
check mf2-wasm-csr -p mf2 --features native,leptos,csr --target wasm32-unknown-unknown
check shim-wasm-hydrate -p mf2-native --features mf2/leptos,mf2/hydrate --target wasm32-unknown-unknown
check mf2-host-hydrate -p mf2 --features native,leptos,hydrate
check mf2-host-ssr -p mf2 --features native,leptos,ssr
