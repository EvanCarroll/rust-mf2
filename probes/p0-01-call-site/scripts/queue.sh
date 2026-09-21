#!/usr/bin/env bash
# P0.1: build apps one after another (never two builds at once), skipping any
# (workload, template) pair already in the results TSV, so the queue can be
# re-run to resume. Usage: scripts/queue.sh <results.tsv> <app-dir>...
set -uo pipefail
out=$1; shift
here=$(dirname "$0")
touch "$out"
for app in "$@"; do
  wl=$(basename "$(dirname "$(realpath "$app")")"); tpl=$(basename "$app"); tpl=${tpl#app-}
  if awk -F'\t' -v w="$wl" -v t="$tpl" '$1==w && $2==t {f=1} END {exit !f}' "$out"; then
    echo "skip $wl/$tpl"; continue
  fi
  echo "=== $(date +%T) $wl/$tpl"
  "$here/build-app.sh" "$app" "$out" 2>&1 | grep -v -e proc-macro-error2 -e future-incompat || echo "FAILED $wl/$tpl"
done
echo "=== done $(date +%T)"
