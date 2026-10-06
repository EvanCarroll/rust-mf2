#!/bin/sh
# The provenance of every figure a run produces: the tools that made it.
# tools/ci/setup.sh prints this at the end of each job, and tools/ci/report.sh
# puts it in the report's header — a size claim is worth nothing without the
# versions that produced it.
#
# The container image's digest is not exposed to a job, so the image reference
# and these versions are what attributes a figure.
set -eu
one() { "$@" 2>&1 | head -1 || true; }
echo "--- versions ---"
echo "host: $(rustc -vV | sed -n 's/^host: //p')"
echo "rustc: $(one rustc -V)"
echo "cargo: $(one cargo -V)"
echo "wasm-opt: $(one wasm-opt --version)"
echo "wasm-bindgen: $(one wasm-bindgen --version)"
echo "brotli: $(one brotli --version)"
echo "node: $(one node -v)"
echo "alpine: $(cat /etc/alpine-release 2>/dev/null || echo unknown)"
echo "----------------"
