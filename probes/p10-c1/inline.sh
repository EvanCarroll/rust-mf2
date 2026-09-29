#!/usr/bin/env bash
# Phase 10 C1: the inline short string of 19 §7 ("the `&str` copy"),
# measured against C1 in one tree and one lock, then taken out again.
#
#   bash probes/p10-c1/inline.sh
#
# Run from the main tree with C1's change in place, after
# `bash probes/p10-b2/measure.sh c1` (which leaves the size workloads and
# their locks in target/p10-b2/) and after `cargo xtask tui-gate
# --save-baseline target/p10-c1/tui-c1`. It applies inline-str.patch (a
# `Text::Inline` holding up to 22 bytes on 64-bit targets and 10 on
# `wasm32`, which `IntoArg for &str` and `for char` fill), runs the
# `arguments` test, `cargo xtask size --keep`, `cargo xtask b5 --view
# --keep` and `cargo xtask tui-gate --baseline target/p10-c1/tui-c1`, and
# reverses the patch, whatever happened. Logs in target/p10-c1/inline/.
#
# One build at a time; CARGO_BUILD_JOBS=3.
set -u
cd "$(dirname "$0")/../.."
root=$(pwd)
logs=$root/target/p10-c1/inline
mkdir -p "$logs"
patch=probes/p10-c1/inline-str.patch
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=3

stamp() { echo "$(date -Is) $*" | tee -a "$logs/timeline.txt"; }
run() {
  local name=$1; shift
  stamp "start $name: $*"
  "$@" >"$logs/$name.log" 2>&1
  local rc=$?
  stamp "end $name rc=$rc"
  return $rc
}

git apply "$patch" || exit 1
trap 'git apply -R "$root/$patch" && stamp "patch reversed"' EXIT
stamp "patch applied"
run test cargo test -p mf2 --features compile,host-std,fn-datetime --test arguments
run size cargo xtask size --out "$root/target/p10-b2/size" --keep
cp "$root/target/p10-b2/size/report.md" "$logs/size-report.md"
run b5v cargo xtask b5 --view --out "$root/target/p10-b2/b5v" --keep
run tui cargo xtask tui-gate --baseline "$root/target/p10-c1/tui-c1"
stamp "done"
