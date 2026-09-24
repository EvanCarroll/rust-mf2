#!/bin/sh
# tools/watch-edit.sh EXAMPLE PORT [expect-ignored]
#
# The dev loop's check (plans/15-phase-7-work-order.md A6): starts
# `cargo leptos watch` in examples/EXAMPLE (demo-ssr: 3702, demo-islands:
# 3704), edits fr's `reset` text, and requires the restarted server to serve
# the edit, then the revert. With `expect-ignored` it requires instead that
# no rebuild follows the edit within 90 s — the control, for a manifest
# without `watch-additional-files`. The log goes to target/watch-edit/.
set -u
ex=$1; port=$2; mode=${3:-expect-seen}
root=$(cd "$(dirname "$0")/.." && pwd)
dir=$root/examples/$ex; file=$dir/i18n/locales/fr/main.mf2
mkdir -p "$root/target/watch-edit"
log=$root/target/watch-edit/$ex-$mode.log
cd "$dir" || exit 2
CARGO_BUILD_JOBS=2 cargo leptos watch >"$log" 2>&1 &
pid=$!
listens() { grep -c 'listening on' "$log"; }
wait_listens() { # N SECONDS
  i=0; while [ "$(listens)" -lt "$1" ]; do
    i=$((i+1)); [ $i -gt "$2" ] && return 1; sleep 1; done; }
page() { curl -s -H 'Accept-Language: fr' -H 'Cookie: mf2_locale=fr' "http://127.0.0.1:$port/?lang=fr"; }
fail=0
wait_listens 1 1500 || { echo "$ex: never listened"; kill $pid; exit 2; }
page | grep -c 'Réinitialiser' | sed "s/^/$ex: before, fr page mentions Réinitialiser x/"
t0=$(date +%s.%N)
sed -i 's/^reset = Réinitialiser$/reset = Réinitialiser A6/' "$file"
if [ "$mode" = expect-ignored ]; then
  if wait_listens 2 90; then echo "$ex: REBUILT (unexpected)"; fail=1
  else echo "$ex: no rebuild in 90 s after the edit (as expected without the setting)"; fi
else
  if wait_listens 2 300; then
    t1=$(date +%s.%N); sleep 1
    echo "$ex: restarted $(echo "$t1 - $t0" | bc) s after the edit"
    if page | grep -q 'Réinitialiser A6'; then echo "$ex: PASS edit served"; else echo "$ex: FAIL edit not served"; fail=1; fi
  else echo "$ex: FAIL no restart within 300 s"; fail=1; fi
fi
sed -i 's/^reset = Réinitialiser A6$/reset = Réinitialiser/' "$file"
if [ "$mode" != expect-ignored ]; then
  if wait_listens 3 300; then sleep 1
    if page | grep -q 'Réinitialiser A6'; then echo "$ex: FAIL revert not served"; fail=1; else echo "$ex: PASS revert served"; fi
  else echo "$ex: FAIL no restart after revert"; fail=1; fi
fi
grep -E 'Compiling|Dirty' "$log" | sed 's/^/  /' | sort | uniq -c | tail -8
pkill -P $pid; kill $pid 2>/dev/null; wait $pid 2>/dev/null
exit $fail
