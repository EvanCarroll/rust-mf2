#!/usr/bin/env bash
# P0.9: (re)generate the reference workload with the probe's three data-only
# templates and sync it into the probe workspace:
#   gen/locales/        → crates/i18n/locales/   (the i18n crate's sources)
#   gen/app-<t>/src/    → apps/<t>/src/          (t = tr, trivial, direct)
#   gen/app-<t>/style/  → apps/<t>/style/
# The apps' Cargo.toml and the server (apps/tr/server/main.rs) are the
# probe's own; the generated Cargo.toml / src/main.rs are not used.
#
# Usage: scripts/regen.sh            (from anywhere; knobs: M=2000 sites, seed 1)
set -euo pipefail
probe="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$probe/../.." && pwd)"
sites="${SITES:-2000}"

cd "$root"
CARGO_BUILD_JOBS=3 cargo xtask gen-workload all -m "$sites" \
    -t "$probe/templates/tr" -t "$probe/templates/trivial" -t "$probe/templates/direct" \
    --out "$probe/gen"

rm -rf "$probe/crates/i18n/locales"
cp -r "$probe/gen/locales" "$probe/crates/i18n/locales"
for t in tr trivial direct; do
    mkdir -p "$probe/apps/$t"
    rm -rf "$probe/apps/$t/src" "$probe/apps/$t/style"
    cp -r "$probe/gen/app-$t/src" "$probe/apps/$t/src"
    cp -r "$probe/gen/app-$t/style" "$probe/apps/$t/style"
    rm -f "$probe/apps/$t/src/main.rs"
done
echo "synced: $(ls "$probe/crates/i18n/locales" | tr '\n' ' ')"
for t in tr trivial direct; do
    printf '%s: ' "$t"; rg -o --no-filename 'tr0?!\(|tr(_args)?\(MsgId' "$probe/apps/$t/src" | wc -l
done
