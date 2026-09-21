#!/usr/bin/env bash
# P0.9 fragility check: move the target directory (as a CI cache restore to
# another path does), touch one app source file, rebuild server and wasm from
# the moved directory. The i18n crate stays "fresh", so its build script does
# not rerun and the baked OUT_DIR path no longer exists: without the macro's
# relocation fallback this failed at every call site (2,003 errors). Moves the
# directory back afterwards. P09_MANIFEST_MODE=inline checks the inline mode.
# Usage: scripts/relocate-check.sh
set -uo pipefail
cd "$(dirname "$0")/.."
build() {
    CARGO_BUILD_JOBS=3 cargo build -v --package=workload-app-tr --bin=workload-app-tr --no-default-features --features=ssr "$@" &&
    CARGO_BUILD_JOBS=3 cargo build -v --package=workload-app-tr --lib --target=wasm32-unknown-unknown --no-default-features --features=hydrate "$@"
}
echo "# mode: ${P09_MANIFEST_MODE:-path}"
build >/dev/null 2>&1 || { echo "initial build failed"; exit 1; }
mv target target-moved
trap 'mv target-moved target' EXIT
touch apps/tr/src/lib.rs
log=$(mktemp)
CARGO_TARGET_DIR="$PWD/target-moved" build >"$log" 2>&1
rc=$?
echo "rebuild (ssr + wasm) from moved target dir: exit=$rc"
grep -E '^\s+(Dirty|Compiling) (p09|workload)' "$log" | sed -E 's/^\s+/  /; s#/home/[^ )]*/probes/p0-09-build-orchestration/##g' | cut -c1-200
grep -E 'Running `.*p09-i18n-[0-9a-f]+/build-script-build`' "$log" | sed -E 's/.*(build\/p09-i18n-[0-9a-f]+).*/  build.rs ran: \1/'
awk '/^error/{p=1} p{print "  | " $0} /^$/{if(p) exit}' "$log" | head -12
grep -c '^error' "$log" | sed 's/^/  errors: /'
rm -f "$log"
