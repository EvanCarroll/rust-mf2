#!/usr/bin/env bash
# P0.11, one command: build, native model (A and B), browser (A and B, 1x and
# 4x CPU throttle, two runs). Timings are wall-clock on a shared machine: re-run
# this on a quiet machine before quoting them as final.
#
#   probes/p0-11-node-update/scripts/run.sh
set -euo pipefail
probe=$(realpath "$(dirname "$0")/..")
"$probe/scripts/build.sh"
"$probe/target/cargo/release/p0-11-native" both | tee "$probe/target/native.txt"
node "$probe/browser/run.mjs" --runs 2 --json "$probe/target/browser.json" | tee "$probe/target/browser.txt"
