#!/usr/bin/env bash
# Phase 10 B2: the web's figures before and after `mf2::native`, in this
# worktree (one tree), each generated application and each demo keeping its
# own Cargo.lock between the runs. B1's `probes/p10-b1/measure.sh`, with
# every built wasm and catalog hashed, since B2 changes no web code and the
# claim to check is "byte-identical", not "within the gate".
#
#   bash probes/p10-b2/measure.sh base   # at the commit before B2
#   bash probes/p10-b2/measure.sh b2     # with B2's change applied, reusing base's locks
#
# One build at a time; CARGO_BUILD_JOBS=3 for cargo, 2 for cargo-leptos.
set -u
label=${1:?base or b2}
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b2
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
# Every file under DIR matching NAME, hashed, paths relative to DIR.
hashes() {
  ( cd "$1" && find . -type f -name "$2" -not -path '*/target-apps/*' -print0 | sort -z | xargs -0 sha256sum )
}

git log --oneline -1 >"$logs/commit.txt"
git status --short >"$logs/tree-status.txt"
sha256sum Cargo.lock >"$logs/workspace-lock.sha256"

# Restore the demos' locks the base run resolved, byte for byte.
if [ "$label" != base ]; then
  for d in demo-ssr demo-islands demo-csr; do
    cp "$out/locks/base/$d.Cargo.lock" "examples/$d/Cargo.lock"
  done
fi

CARGO_BUILD_JOBS=3 run size cargo xtask size --out "$out/size" "${keep[@]}"
hashes "$out/size" '*.wasm' >"$logs/size-wasm.sha256"
find "$out/size" -name Cargo.lock -not -path '*/target-apps/*' -print0 | sort -z | xargs -0 sha256sum >"$logs/size-locks.sha256"
cp "$out/size/report.md" "$logs/size-report.md"
CARGO_BUILD_JOBS=3 run b5v cargo xtask b5 --view --out "$out/b5v" "${keep[@]}"
hashes "$out/b5v" '*.wasm' >"$logs/b5v-wasm.sha256"
CARGO_BUILD_JOBS=3 run catalog-size cargo xtask catalog-size
mkdir -p "$out/catalog-bench/$label"
cp target/catalog-bench/size.json target/catalog-bench/size.md "$out/catalog-bench/$label/"

demo() {
  local d=$1; shift
  local dest=$out/demos/$label/$d
  rm -rf "$dest"; mkdir -p "$dest"
  ( cd "examples/$d" && "$@" ) >"$logs/demo-$d.log" 2>&1
  local rc=$?
  stamp "built $d rc=$rc"
  case $d in
    demo-csr) cp -a "examples/$d/dist" "$dest/dist"; node probes/p10-names/measure-demo.mjs "$dest/dist" "$d $label" >"$dest/measure.md"
              hashes "$dest/dist" '*' >"$logs/demo-csr-dist.sha256" ;;
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
