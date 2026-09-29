#!/usr/bin/env bash
# Phase 10 C2: the checks of C2's done-when list, run one after the other in
# the main tree with C2's change, each logged with its exit status and wall
# time (taken under whatever load the machine had). B2's
# `probes/p10-b2/checks.sh` with C2's labels and log directory.
#
#   bash probes/p10-c2/checks.sh [NAME…]   # all, or the named ones
#
# Logs in target/p10-c2/checks/. `cargo xtask ci` is run on its own, before
# committing. The browser checks are B1's `probes/p10-b1/e2e.sh`, on the
# tree (Leptos 0.9, label `c2-leptos-0-9`) and on copies made by
# `probes/p10-names/demos-0-8.py` (Leptos 0.8, `c2-leptos-0-8`); the 0.8
# copies take over the build directories of the copies made at HEAD
# (`target/a7-demo-0-8/c2-head`), so that only what changed is rebuilt.
set -u
cd "$(dirname "$0")/../.."
out=target/p10-c2/checks
mkdir -p "$out"
export CARGO_BUILD_JOBS=3
check() {
  local name=$1; shift
  local start=$(date +%s)
  echo "$(date -Is) start $name: $*" | tee -a "$out/timeline.txt"
  "$@" >"$out/$name.log" 2>&1
  local rc=$?
  echo "$(date -Is) end $name rc=$rc ($(( $(date +%s) - start )) s)" | tee -a "$out/timeline.txt"
}
copies_0_8() {
  python3 probes/p10-names/demos-0-8.py c2 || return 1
  for d in demo-ssr demo-islands demo-csr; do
    if [ -d "target/a7-demo-0-8/c2-head/examples/$d/target" ]; then
      mv "target/a7-demo-0-8/c2-head/examples/$d/target" "target/a7-demo-0-8/c2/examples/$d/target"
    fi
  done
  bash probes/p10-b1/e2e.sh target/a7-demo-0-8/c2 c2-leptos-0-8
}
all=(docs docs-rs codegen-matrix scenarios leptos-0-8 l6-web l7-web churn msrv b12 b12-generated e2e-0-9 e2e-0-8)
for name in "${@:-${all[@]}}"; do
  case $name in
    docs) check docs cargo xtask docs ;;
    docs-rs) check docs-rs cargo xtask docs-rs ;;
    codegen-matrix) check codegen-matrix cargo xtask codegen-matrix ;;
    scenarios) check scenarios cargo xtask scenarios ;;
    leptos-0-8) check leptos-0-8 cargo xtask leptos-0-8 ;;
    l6-web) check l6-web cargo xtask l6-web ;;
    l7-web) check l7-web cargo xtask l7-web ;;
    churn) check churn cargo xtask churn ;;
    msrv) check msrv cargo xtask msrv ;;
    b12) check b12 bash bench/b12/check.sh ;;
    b12-generated) check b12-generated cargo xtask b12-generated ;;
    e2e-0-9) check e2e-0-9 bash probes/p10-b1/e2e.sh . c2-leptos-0-9 ;;
    e2e-0-8) check e2e-0-8 copies_0_8 ;;
    *) echo "unknown check $name" >&2 ;;
  esac
done
