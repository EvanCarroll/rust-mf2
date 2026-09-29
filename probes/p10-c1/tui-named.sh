#!/usr/bin/env bash
# Phase 10 C1: `examples/tui`'s `tui-mf2` built as `cargo xtask tui-gate`
# builds it (release), but with its symbols kept, at HEAD's sources and at
# C1's, and the functions whose size moved listed (`nm --size-sort`, names
# demangled, crate hashes dropped): the reading of what C1 moved in the
# stripped binary.
#
#   bash probes/p10-c1/tui-named.sh
#
# Run from the main tree with C1's change in place. C1's crate sources are
# kept aside, HEAD's written in their place for the first build, and C1's put
# back (with fresh modification times) for the second, whatever happens.
# Outputs in target/p10-c1/tui-named/; its build directory `t` is removed at
# the end.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-c1/tui-named
changed="crates/mf2/src/lib.rs crates/mf2/src/leptos/signal.rs crates/mf2-macros/src/expand.rs"
added="crates/mf2/src/into_arg.rs crates/mf2/src/__arg.rs"
export CARGO_NET_OFFLINE=true
mkdir -p "$out"
tar -cf "$out/c1-src.tar" $changed $added
restore() { tar -xmf "$out/c1-src.tar"; }
trap restore EXIT

build() {
  local label=$1
  CARGO_PROFILE_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 cargo build --release --bin tui-mf2 \
    --manifest-path examples/tui/Cargo.toml --target-dir "$out/t" --offline >"$out/$label.log" 2>&1
  cp "$out/t/release/tui-mf2" "$out/$label"
  nm --size-sort -S -C "$out/$label" \
    | awk '{ $1 = ""; print }' \
    | sed -E 's/\[[0-9a-f]{16}\]//g; s/::h[0-9a-f]{16}$//' >"$out/$label.syms"
  size -A "$out/$label" >"$out/$label.sections"
}

for f in $changed; do git show "HEAD:$f" >"$f"; done
rm -f $added
build base
restore
build c1
python3 - "$out/base.syms" "$out/c1.syms" >"$out/diff.txt" <<'EOF'
import collections, sys
def load(path):
    sizes = collections.Counter()
    for line in open(path):
        parts = line.split(None, 2)
        if len(parts) == 3:
            sizes[parts[2].strip()] += int(parts[0], 16)
    return sizes
base, c1 = load(sys.argv[1]), load(sys.argv[2])
delta = {n: c1[n] - base[n] for n in set(base) | set(c1) if c1[n] != base[n]}
print(f"symbols: net {sum(delta.values()):+d} B over {len(delta)} names")
for n, d in sorted(delta.items(), key=lambda x: -abs(x[1]))[:40]:
    print(f"{d:+7d}  base {base[n]:6d}  c1 {c1[n]:6d}  {n[:160]}")
EOF
rm -rf "$out/t"
echo done
