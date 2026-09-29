#!/usr/bin/env bash
# Phase 10 C1: the shipped clients of the two size workloads that call `tr!`
# (`tr` for B1 and B5, `tr-view` for `b5 --view`), at both scales, built from
# HEAD's sources and from C1's in one tree, with the workloads and their locks
# kept, and every module kept for a byte-level reading: raw, gzip -9 and
# brotli q11, and `probes/p10-b4/wasmcmp.py` section by section.
#
#   bash probes/p10-c1/ab.sh
#
# Run from the main tree with C1's change in place, after
# `bash probes/p10-b2/measure.sh c1`. C1's crate sources are kept aside,
# HEAD's written in their place for the first two builds, and C1's put back
# (with fresh modification times, so that cargo rebuilds) for the last two,
# whatever happens. Outputs in target/p10-c1/ab/.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-c1/ab
changed="crates/mf2/src/lib.rs crates/mf2/src/leptos/signal.rs crates/mf2-macros/src/expand.rs"
added="crates/mf2/src/into_arg.rs crates/mf2/src/__arg.rs"
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=3
mkdir -p "$out"
tar -cf "$out/c1-src.tar" $changed $added
restore() { tar -xmf "$out/c1-src.tar"; }
trap restore EXIT

build() {
  local label=$1
  cargo xtask size --out "$root/target/p10-b2/size" --keep >"$out/$label-size.log" 2>&1
  cargo xtask b5 --view --out "$root/target/p10-b2/b5v" --keep >"$out/$label-b5v.log" 2>&1
  for n in 1860 3720; do
    cp "$root/target/p10-b2/size/wl-$n/pkg-tr/opt.wasm" "$out/$label-tr-$n.wasm"
    cp "$root/target/p10-b2/b5v/wl-view-$n/pkg-tr-view/opt.wasm" "$out/$label-tr-view-$n.wasm"
  done
}

for f in $changed; do git show "HEAD:$f" >"$f"; done
rm -f $added
build base
restore
build c1

{
  echo "| module | base raw / gz / br | C1 raw / gz / br | delta raw / gz / br |"
  echo "|---|---:|---:|---:|"
  for m in tr-1860 tr-3720 tr-view-1860 tr-view-3720; do
    python3 - "$out/base-$m.wasm" "$out/c1-$m.wasm" "$m" <<'EOF'
import brotli, gzip, sys
def sizes(p):
    b = open(p, "rb").read()
    return len(b), len(gzip.compress(b, 9, mtime=0)), len(brotli.compress(b, quality=11))
a, c, m = sizes(sys.argv[1]), sizes(sys.argv[2]), sys.argv[3]
fmt = lambda t: " / ".join(f"{x:,}" for x in t)
print(f"| {m} | {fmt(a)} | {fmt(c)} | {' / '.join(f'{y - x:+,}' for x, y in zip(a, c))} |")
EOF
  done
  for m in tr-1860 tr-3720 tr-view-1860 tr-view-3720; do
    echo
    echo "## $m"
    python3 probes/p10-b4/wasmcmp.py "$out/base-$m.wasm" "$out/c1-$m.wasm"
  done
} >"$out/report.md"
sha256sum "$out"/*.wasm >"$out/wasm.sha256"
echo done
