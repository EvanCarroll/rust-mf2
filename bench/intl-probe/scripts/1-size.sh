#!/usr/bin/env bash
# Item 1: wasm and JS glue, raw and gzip -9, per variant, as deltas against `base`.
exec "$(dirname "$0")/build.sh" "$@"
