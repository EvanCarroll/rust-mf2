#!/usr/bin/env bash
# Phase 10 A7 item 4: coherence of the call-site types with Leptos and
# Ratatui both on — the intended impls, then each rule's negative control.
# Run from anywhere; the output is the observation.
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
export CARGO_BUILD_JOBS=2
run() {
  local dir="$1"
  shift
  echo "=== ($(basename "$dir")) cargo $*"
  (cd "$dir" && cargo "$@" 2>&1) | grep -E "^error|^\s+--> |Finished|could not compile|^\s+= note: (conflicting|upstream)|^\s+\|\s+-+ first implementation|^\s+= note: downstream" | head -14
}
# The impls themselves, in the library: no Leptos, the server, the client.
run "$root" check -p leptos-mf2 --no-default-features --features ratatui
run "$root" check -p leptos-mf2 --features ssr,ratatui
run "$root" check -p leptos-mf2 --features hydrate,ratatui --target wasm32-unknown-unknown
# What an application writes, beside the Leptos glue: server, client.
run "$here" check -p coherence
run "$here" check -p coherence --no-default-features --features hydrate
# The rules' negative controls.
run "$here" check -p coherence --features probe-cow
run "$here" check -p coherence --features probe-fromiter
run "$here" check -p coherence --features probe-cell
run "$here" check -p coherence --features probe-listitem
