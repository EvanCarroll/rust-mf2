#!/usr/bin/env bash
# Phase 10 B1: the size A/B, in this worktree (one tree), each generated
# application and each demo keeping its own Cargo.lock between the runs.
#
#   bash probes/p10-b1/measure.sh base   # at the commit before B1
#   bash probes/p10-b1/measure.sh b1     # at B1's commit, reusing base's locks
#
# One build at a time; CARGO_BUILD_JOBS=3 for cargo, 2 for cargo-leptos.
set -u
label=${1:?base or b1}
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b1
logs=$out/logs/$label
mkdir -p "$logs" "$out/locks"
export CARGO_NET_OFFLINE=true
keep=()
if [ "$label" != base ]; then keep=(--keep); fi

stamp() { echo "$(date -Is) $*" | tee -a "$logs/timeline.txt"; }
run() {
  local name=$1; shift
  stamp "start $name: $*"
  "$@" >"$logs/$name.log" 2>&1
  local rc=$?
  stamp "end $name rc=$rc"
  return $rc
}

git log --oneline -1 >"$logs/commit.txt"

# Restore the demos' locks the base run resolved, byte for byte.
if [ "$label" != base ]; then
  for d in demo-ssr demo-islands demo-csr; do
    cp "$out/locks/base/$d.Cargo.lock" "examples/$d/Cargo.lock"
  done
fi

CARGO_BUILD_JOBS=3 run size cargo xtask size --out "$out/size" "${keep[@]}"
CARGO_BUILD_JOBS=3 run b5v cargo xtask b5 --view --out "$out/b5v" "${keep[@]}"
CARGO_BUILD_JOBS=3 run catalog-size cargo xtask catalog-size

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
stamp "start demos"
CARGO_BUILD_JOBS=2 demo demo-ssr cargo leptos build --release --split --frontend-only --cargo-offline
CARGO_BUILD_JOBS=2 demo demo-islands cargo leptos build --release --frontend-only --cargo-offline
CARGO_BUILD_JOBS=3 demo demo-csr trunk build --release
if [ "$label" = base ]; then
  mkdir -p "$out/locks/base"
  for d in demo-ssr demo-islands demo-csr; do
    cp "examples/$d/Cargo.lock" "$out/locks/base/$d.Cargo.lock"
  done
fi
sha256sum examples/demo-*/Cargo.lock >"$logs/demo-locks.sha256"
stamp "done $label"
