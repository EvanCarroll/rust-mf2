#!/usr/bin/env bash
# Phase 10 B1: the six browser checks of the demos (demo, lazy, islands, csr,
# zone, a11y), in Chromium and Firefox, against release builds.
#
#   bash probes/p10-b1/e2e.sh ROOT LABEL
#
# ROOT holds `examples/` and `tools/e2e/`: the tree itself (Leptos 0.9), or
# a copy made by probes/p10-names/demos-0-8.py (Leptos 0.8).
set -u
root=$(cd "${1:?root}" && pwd)
label=${2:?label}
out=$(cd "$(dirname "$0")/../.." && pwd)/target/p10-b1/e2e/$label
mkdir -p "$out"
export CARGO_NET_OFFLINE=true
stamp() { echo "$(date -Is) $*" | tee -a "$out/timeline.txt"; }

build() {
  local d=$1; shift
  ( cd "$root/examples/$d" && "$@" ) >"$out/build-$d.log" 2>&1
  stamp "built $d rc=$?"
}
stamp "start $label ($root)"
CARGO_BUILD_JOBS=2 build demo-ssr cargo leptos build --release --split --cargo-offline
CARGO_BUILD_JOBS=2 build demo-islands cargo leptos build --release --cargo-offline
CARGO_BUILD_JOBS=3 build demo-csr trunk build --release

serve() {
  local d=$1 bin=$2 port=$3
  ( cd "$root/examples/$d" && LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
      LEPTOS_SITE_ADDR=127.0.0.1:$port exec ./target/release/$bin ) >"$out/serve-$d.log" 2>&1 &
  echo $!
}
ssr_pid=$(serve demo-ssr demo-ssr 3702)
isl_pid=$(serve demo-islands demo-islands 3704)
for port in 3702 3704; do
  for _ in $(seq 1 60); do
    curl -fs -o /dev/null "http://127.0.0.1:$port/" && break
    sleep 1
  done
done
stamp "servers up (ssr $ssr_pid, islands $isl_pid)"

check() {
  local name=$1; shift
  ( cd "$root/tools/e2e" && node run.mjs "$name" "$@" --browser chromium,firefox ) >"$out/check-$name.log" 2>&1
  local rc=$?
  stamp "check $name rc=$rc: $(grep -cE '^\[[a-z]+\] PASS' "$out/check-$name.log") PASS, $(grep -cE '^\[[a-z]+\] FAIL' "$out/check-$name.log") FAIL; $(grep -E 'assertions passed' "$out/check-$name.log" | tail -1)"
}
check demo --base-url http://127.0.0.1:3702
check lazy --base-url http://127.0.0.1:3702
check islands --base-url http://127.0.0.1:3704
check csr
check zone --base-url http://127.0.0.1:3702
MF2_ISLANDS_URL=http://127.0.0.1:3704 check a11y --base-url http://127.0.0.1:3702

kill "$ssr_pid" "$isl_pid" 2>/dev/null
wait 2>/dev/null
stamp "done $label"
