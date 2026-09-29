#!/usr/bin/env bash
# Phase 10 B1: the three demos only, built as they ship and measured, with
# the base run's locks restored byte for byte. For a variant of the
# components' dispatch (the function table), applied to the working tree
# before this runs and reverted after.
#
#   bash probes/p10-b1/measure-demos.sh LABEL
set -u
label=${1:?label}
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b1
logs=$out/logs/$label
mkdir -p "$logs"
export CARGO_NET_OFFLINE=true

stamp() { echo "$(date -Is) $*" | tee -a "$logs/timeline.txt"; }

git log --oneline -1 >"$logs/commit.txt"
git status --short -- crates >"$logs/tree-status.txt"
for d in demo-ssr demo-islands demo-csr; do
  cp "$out/locks/base/$d.Cargo.lock" "examples/$d/Cargo.lock"
done

demo() {
  local d=$1; shift
  local dest=$out/demos/$label/$d
  rm -rf "$dest"; mkdir -p "$dest"
  ( cd "examples/$d" && "$@" ) >"$logs/demo-$d.log" 2>&1
  local rc=$?
  stamp "built $d rc=$rc"
  case $d in
    demo-csr) cp -a "examples/$d/dist" "$dest/dist"; node probes/p10-names/measure-demo.mjs "$dest/dist" "$d $label" >"$dest/measure.md" ;;
    *) cp -a "examples/$d/target/site/pkg" "$dest/pkg"; node probes/p10-names/measure-demo.mjs "$dest/pkg" "$d $label" >"$dest/measure.md" ;;
  esac
  cat "$dest/measure.md" >>"$logs/timeline.txt"
}
stamp "start demos ($label)"
CARGO_BUILD_JOBS=2 demo demo-ssr cargo leptos build --release --split --frontend-only --cargo-offline
CARGO_BUILD_JOBS=2 demo demo-islands cargo leptos build --release --frontend-only --cargo-offline
CARGO_BUILD_JOBS=3 demo demo-csr trunk build --release
sha256sum examples/demo-*/Cargo.lock >"$logs/demo-locks.sha256"
stamp "done $label"
