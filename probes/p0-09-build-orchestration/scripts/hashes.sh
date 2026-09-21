#!/usr/bin/env bash
# P0.9: every i18n OUT_DIR under target/ (the server build's and the wasm
# build's): manifest sha256, MANIFEST_HASH in the generated module, catalog
# file names, build.rs run time; then the site's wasm and the server binary.
set -uo pipefail
cd "$(dirname "$0")/.."
for f in $(find target -path '*p09-i18n-*/out/manifest.mf2m' 2>/dev/null | sort); do
    d=$(dirname "$f")
    printf '%s  manifest-sha=%s  %s  cats=[%s]  build.rs=%sµs  mtime=%s\n' \
        "${d#target/}" "$(sha256sum "$f" | cut -c1-12)" \
        "$(grep -o 'MANIFEST_HASH: u64 = 0x[0-9a-f]*' "$d/mf2_generated.rs" | sed 's/.*= //')" \
        "$(cd "$d" && ls *.mf2b | tr '\n' ' ' | sed 's/ $//')" \
        "$(sed -n 's/build_rs_micros //p' "$d/mf2-build-report.txt")" \
        "$(date -r "$d/mf2-build-report.txt" +%T)"
done
for f in target/site/pkg/*.wasm target/wasm32-unknown-unknown/debug/workload_app_tr.wasm target/debug/workload-app-tr target/release/workload-app-tr; do
    [ -f "$f" ] && printf '%s  sha=%s  %s bytes  %s\n' "$f" "$(sha256sum "$f" | cut -c1-12)" "$(stat -c %s "$f")" "$(date -r "$f" +%T)"
done
exit 0
