#!/usr/bin/env bash
# P0.9: every incremental-rebuild scenario in one run, under plain cargo or
# cargo-leptos. Prints per step: what rebuilt, errors, the manifest hash and
# catalog names in each i18n OUT_DIR the step's build script wrote, and what
# the server renders.
# Usage: scripts/scenarios.sh plain|leptos
set -uo pipefail
cd "$(dirname "$0")/.."
mode="${1:?plain|leptos}"
marker=target/p09-scenario-marker
b() { touch "$marker"; if [ "$mode" = plain ]; then scripts/build-plain.sh both; else scripts/build-leptos.sh; fi; }
outdirs() {
    local any=no
    for r in $(find target -path '*p09-i18n-*/out/mf2-build-report.txt' -newer "$marker" 2>/dev/null | sort); do
        d=$(dirname "$r"); any=yes
        printf '  OUT_DIR %s: MANIFEST_HASH %s, manifest sha %s, catalogs %s\n' "${d#target/}" \
            "$(grep -o 'MANIFEST_HASH: u64 = 0x[0-9a-f]*' "$d/mf2_generated.rs" | sed 's/.*= //')" \
            "$(sha256sum "$d/manifest.mf2m" | cut -c1-12)" "$(cd "$d" && ls *.mf2b | tr '\n' ' ')"
    done
    [ $any = no ] && echo "  (no i18n build script ran)"
}
wasm() { for f in target/site/pkg/workload-app-tr.wasm target/wasm32-unknown-unknown/debug/workload_app_tr.wasm; do [ -f "$f" ] && [ "$f" -nt "$marker" ] && echo "  wasm $f sha $(sha256sum "$f" | cut -c1-12)"; done; }
step() { echo; echo "== $*"; }
step "S0 baseline (revert)"; scripts/edit.sh revert >/dev/null; b; outdirs; wasm
step "S1 no change"; b; outdirs
step "S2 touch en/common.mf2 (mtime only)"; scripts/edit.sh touch >/dev/null; b; outdirs; wasm
step "S3 edit the text of activity-available (en)"; scripts/edit.sh text >/dev/null; b; outdirs; wasm
scripts/serve-check.sh debug "Result than page still screenshots EDITED-P09." "Result than page still screenshots." | grep -E 'server manifest|count' | sed 's/^/  /'
step "S4 add {\$extra} to actions-joined-busy"; scripts/edit.sh addvar >/dev/null; b; outdirs
step "S4b fix the second crate's call site (app call sites still stale)"
sed -i 's/tr!("actions-joined-busy", title = who)/tr!("actions-joined-busy", title = who, extra = "!")/' crates/second/src/lib.rs; b
step "S5 revert"; scripts/edit.sh revert >/dev/null; b; outdirs; wasm
step "S6 call tr!(\"p09.added-message\") in the second crate (id not in locales)"; scripts/edit.sh addsite >/dev/null; b
step "S7 add p09.added-message to en"; scripts/edit.sh addmsg >/dev/null; b; outdirs; wasm
scripts/serve-check.sh debug "Freshly added for Ada." "Disabled accessibility add no details block delete." | grep -E 'server manifest|Freshly|count' | sed 's/^/  /'
step "S8 edit activity-available in pl only"; scripts/edit.sh pl >/dev/null; b; outdirs; wasm
P09_LOCALE=pl scripts/serve-check.sh debug "PL-EDITED-P09 gonąmo." | grep -E 'count' | sed 's/^/  /'
step "S9 revert"; scripts/edit.sh revert >/dev/null; b; outdirs
