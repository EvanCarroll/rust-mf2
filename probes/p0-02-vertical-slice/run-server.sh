#!/bin/sh
# Serve an already-built probe (debug or release) without rebuilding.
# usage: ./run-server.sh [debug|release]   (address: 127.0.0.1:3702)
set -eu
cd "$(dirname "$0")"
profile="${1:-debug}"
export LEPTOS_OUTPUT_NAME=p002 LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
       LEPTOS_SITE_ADDR="${LEPTOS_SITE_ADDR:-127.0.0.1:3702}" LEPTOS_RELOAD_PORT=3703 LEPTOS_ENV=PROD
exec "target/$profile/p002"
