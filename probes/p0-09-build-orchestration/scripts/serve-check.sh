#!/usr/bin/env bash
# P0.9: start the built server, fetch /manifest, /catalogs, /second and every
# route, count occurrences of each TEXT argument in the SSR HTML, stop.
# Usage: scripts/serve-check.sh [debug|release] [TEXT…]   (P09_LOCALE=pl to render pl)
set -uo pipefail
cd "$(dirname "$0")/.."
profile="${1:-debug}"; shift || true
export LEPTOS_OUTPUT_NAME=workload-app-tr LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
       LEPTOS_SITE_ADDR=127.0.0.1:3709 LEPTOS_RELOAD_PORT=3710 LEPTOS_ENV=PROD
"target/$profile/workload-app-tr" >/tmp/p09-server.$$.log 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null; rm -f /tmp/p09-server.$$.log' EXIT
for _ in $(seq 1 100); do curl -sf -o /dev/null http://127.0.0.1:3709/manifest && break; sleep 0.1; done
echo "server manifest: $(curl -s http://127.0.0.1:3709/manifest)"
echo "server catalogs: $(curl -s http://127.0.0.1:3709/catalogs | tr '\n' ' ')"
echo "/second:"; curl -s http://127.0.0.1:3709/second | sed 's/^/    /'
html=$(for r in "" r1 r2 r3; do curl -s "http://127.0.0.1:3709/$r"; done)
echo "SSR HTML bytes (4 routes): ${#html}; meta: $(grep -o '<meta name="mf2-manifest" content="[0-9a-f]*"' <<<"$html" | head -1)"
for t in "$@"; do
    printf 'count(%q) = %s\n' "$t" "$(grep -oF -- "$t" <<<"$html" | wc -l)"
done
