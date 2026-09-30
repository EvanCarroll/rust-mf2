#!/usr/bin/env bash
# Phase 10 B4: the size workloads that `tools/checks/measure.sh` keeps
# (target/p10-b2/size and target/p10-b2/b5v), generated again in place from
# the tree's templates, as `cargo xtask size` and `cargo xtask b5 --view`
# generate them. The generator replaces only what it writes (each app's
# manifest and sources), so every app keeps its Cargo.lock and its build
# directory. `--keep` alone reuses a workload as it stands, so it cannot
# measure a change of template: run this first, then `measure.sh LABEL`.
#
#   bash tools/checks/regen.sh LABEL
#
# Each workload's files but its build output are hashed before and after,
# and their difference kept: logs in target/p10-b2/logs/LABEL/. One build
# at a time; CARGO_BUILD_JOBS=3.
set -u
label=${1:?label}
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-b2
logs=$out/logs/$label
templates=$root/bench/workload-gen/templates
mkdir -p "$logs"
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}

files() {
  ( cd "$1" && find . -type f -not -path './target-apps/*' -not -path './pkg-*' -print0 \
      | sort -z | xargs -0 sha256sum )
}

# DIR SITES MESSAGES COMPONENTS TEMPLATE… — b5's knobs and template order.
regen() {
  local dir=$1 sites=$2 messages=$3 components=$4; shift 4
  local name
  name=$(basename "$dir")
  [ -f "$dir/.workload-gen" ] || { echo "$dir: no kept workload (run measure.sh first)"; return 1; }
  files "$dir" >"$logs/$name.before.sha256"
  cargo run --release --quiet -p workload-gen -- all "$@" \
    -m "$sites" -n "$messages" -k "$components" --out "$dir" >"$logs/$name.log" 2>&1 || return 1
  files "$dir" >"$logs/$name.after.sha256"
  if diff "$logs/$name.before.sha256" "$logs/$name.after.sha256" >"$logs/$name.diff"; then
    echo "$name: every file as it was"
  else
    echo "$name: $(grep -c '^>' "$logs/$name.diff") file(s) changed:"
    grep '^>' "$logs/$name.diff" | awk '{print "  " $3}'
  fi
}

regen "$out/size/wl-1860" 1860 1600 60 -t "$templates/tr" -t idlit -t dummy
regen "$out/size/wl-3720" 3720 3200 120 -t "$templates/tr" -t idlit -t dummy
regen "$out/b5v/wl-view-1860" 1860 1600 60 -t "$templates/tr-view" -t "$templates/idlit-view" -t dummy
regen "$out/b5v/wl-view-3720" 3720 3200 120 -t "$templates/tr-view" -t "$templates/idlit-view" -t dummy
