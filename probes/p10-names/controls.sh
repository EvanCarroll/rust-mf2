#!/usr/bin/env bash
# Phase 10 A7: the naming probe's 0.8 line and its negative controls, each
# a `cargo check` whose errors are the observation. Run from anywhere.
cd "$(dirname "$0")" || exit 1
run() {
  echo "=== cargo check $*"
  CARGO_BUILD_JOBS=2 cargo check "$@" 2>&1 | grep -E "^error|^\s+--> |Finished|could not compile" | head -12
}
run -p naming --no-default-features --features leptos-0-8,axum
run -p naming --no-default-features --features n1-root-rename
run -p naming-n2 --features n2-root-use
run -p naming --features n3-axum-root-use
run -p naming --features n4-axum-root-path
run -p naming-app --features n5-bare-module-import
