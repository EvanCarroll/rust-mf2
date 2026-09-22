#!/usr/bin/env bash
# engines.sh ITEM… — runs probe items in every browser (the Playwright check
# `intl`, `--browser all`: Chromium, Firefox, and WebKit when it can start)
# and, except `speed`, in node (web/node.mjs), then prints their report.
# Items: floor l4 plural ecmaRaw ecmaHandlers edge loc speed.
# Prerequisites: build.sh and data.sh (and `npm install` in tools/e2e).
source "$(dirname "$0")/common.sh"
[ $# -gt 0 ] || { echo "usage: engines.sh ITEM…" >&2; exit 2; }
mkdir -p "$OUT/results"
list=$(IFS=,; echo "$*")
status=0
(cd "$REPO/tools/e2e" && MF2_INTL_ITEMS="$list" node run.mjs intl --browser all \
  --label "$list" --json "$OUT/results/e2e-${list//,/-}.json") || status=$?
sections=()
for item in "$@"; do
  if [ "$item" != speed ]; then
    dst="$OUT/results/node-$item.json"
    [ "$item" = loc ] && dst="$OUT/results/loc-node.json"
    node "$REPO/bench/intl-probe/web/node.mjs" "$item" --out "$dst"
  fi
  case "$item" in
    ecmaRaw|ecmaHandlers) sections+=(ecma) ;;
    *) sections+=("$item") ;;
  esac
done
# Every speed run is kept (timings drift with the clock and the load): the
# report gives the range over runs.
if [[ " $* " == *" speed "* ]]; then
  stamp=$(date +%Y%m%d-%H%M%S)
  for b in chromium firefox webkit; do
    f="$OUT/results/$b.json"
    [ -f "$f" ] && node -e '
      const r = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
      if (r.items.speed) require("fs").writeFileSync(process.argv[2], JSON.stringify({ engine: `${r.browser} ${r.version}`, load: require("os").loadavg(), speed: r.items.speed }) + "\n");
    ' "$f" "$OUT/results/speed-$b-$stamp.json"
  done
fi
echo "machine: $(nproc) CPUs, $(mhz) MHz now, load $(cut -d' ' -f1-3 /proc/loadavg); $(tree_note)" | tee "$OUT/results/machine-${list//,/-}.txt"
node "$REPO/bench/intl-probe/scripts/report.mjs" $(printf '%s\n' "${sections[@]}" | awk '!s[$0]++')
exit "$status"
