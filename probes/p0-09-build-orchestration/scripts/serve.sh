#!/usr/bin/env bash
# P0.9: serve the built app on 127.0.0.1:3709 (foreground) without rebuilding.
# Usage: scripts/serve.sh [debug|release]
set -eu
cd "$(dirname "$0")/.."
export LEPTOS_OUTPUT_NAME=workload-app-tr LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
       LEPTOS_SITE_ADDR=127.0.0.1:3709 LEPTOS_RELOAD_PORT=3710 LEPTOS_ENV=PROD
exec "target/${1:-debug}/workload-app-tr"
