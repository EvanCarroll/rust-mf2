#!/usr/bin/env bash
# P0.9: does rust-analyzer expand tr! (build script → OUT_DIR → baked path →
# proc-macro, through its own proc-macro server)?
#   1. `rust-analyzer diagnostics .` on the probe workspace (all three apps,
#      6,000 tr!/tr0!/direct sites + the second crate): count diagnostics per
#      code; any unresolved-macro-call / unresolved-proc-macro / macro-error
#      would mean the macro was not expanded.
#   2. negative control: put a misspelt id and a wrong argument name into the
#      second crate and run it again — the macro's own compile errors must be
#      reported by rust-analyzer (proof that it ran the proc-macro against the
#      manifest), then restore.
# Usage: scripts/ra-check.sh
set -uo pipefail
cd "$(dirname "$0")/.."
out=target/p09-ra
mkdir -p "$out"
summ() {
    # rust-analyzer prints progress with \r; one diagnostic per "at crate …" record
    tr '\r' '\n' < "$1" | grep -oE '(Error|Warning|WeakWarning|Allow) (Ra|RustcHardError|RustcLint|Clippy)\("[a-z0-9_-]+"' \
        | sort | uniq -c | sed 's/^/   /'
}
t0=$(date +%s)
rust-analyzer diagnostics . >"$out/diag.txt" 2>"$out/diag.err"
echo "[1] rust-analyzer diagnostics . : exit=$? in $(( $(date +%s) - t0 ))s"
summ "$out/diag.txt"
cp crates/second/src/extra.rs "$out/extra.rs.bak"
cat > crates/second/src/extra.rs <<'RS'
//! rust-analyzer negative control (restored by scripts/ra-check.sh).
use p09_i18n::tr;
pub(crate) fn lines(who: &'static str) -> Vec<(String, u32)> {
    let a = tr!("activity-availabel");
    let b = tr!("actions-joined-busy", titel = who);
    let _ = (a, b);
    Vec::new()
}
RS
t0=$(date +%s)
rust-analyzer diagnostics . >"$out/diag-neg.txt" 2>"$out/diag-neg.err"
echo "[2] negative control: exit=$? in $(( $(date +%s) - t0 ))s"
cp "$out/extra.rs.bak" crates/second/src/extra.rs
summ "$out/diag-neg.txt"
tr '\r' '\n' < "$out/diag-neg.txt" | grep -E 'extra.rs: Error' | sed 's/.*file [^:]*\/crates/  crates/' | cut -c1-260
