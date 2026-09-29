#!/usr/bin/env bash
# Phase 10 C2: `examples/tui`'s `tui-mf2` built as `cargo xtask tui-gate`
# builds it (release), but with its symbols kept, from the tree as it stands,
# under LABEL; with two labels, the functions whose size moved between them
# (`nm --size-sort`, names demangled, crate hashes dropped).
#
#   bash probes/p10-c2/tui-named.sh build LABEL     # e.g. head (before C2), c2
#   bash probes/p10-c2/tui-named.sh diff BASE OTHER
#
# Run from the main tree. Outputs in target/p10-c2/tui-named/; its build
# directory `t` is kept between builds (remove it when done).
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out=$root/target/p10-c2/tui-named
export CARGO_NET_OFFLINE=true
mkdir -p "$out"
case ${1:?build or diff} in
build)
  label=${2:?label}
  CARGO_PROFILE_RELEASE_STRIP=none CARGO_BUILD_JOBS=3 cargo build --release --bin tui-mf2 \
    --manifest-path examples/tui/Cargo.toml --target-dir "$out/t" --offline >"$out/$label.log" 2>&1
  cp "$out/t/release/tui-mf2" "$out/$label"
  nm --size-sort -S -C "$out/$label" \
    | awk '{ $1 = ""; print }' \
    | sed -E 's/\[[0-9a-f]{16}\]//g; s/::h[0-9a-f]{16}$//' >"$out/$label.syms"
  size -A "$out/$label" >"$out/$label.sections"
  strip -o "$out/$label.stripped" "$out/$label"
  echo "$label: $(stat -c %s "$out/$label.stripped") B stripped (strip(1); the gate strips through cargo)"
  ;;
diff)
  a=${2:?base}; b=${3:?other}
  python3 - "$out/$a.syms" "$out/$b.syms" <<'PY' | tee "$out/diff-$a-$b.txt"
import collections, sys
def load(path):
    sizes = collections.Counter()
    for line in open(path):
        parts = line.split(None, 2)
        if len(parts) == 3:
            sizes[parts[2].strip()] += int(parts[0], 16)
    return sizes
base, other = load(sys.argv[1]), load(sys.argv[2])
delta = {n: other[n] - base[n] for n in set(base) | set(other) if other[n] != base[n]}
print(f"symbols: net {sum(delta.values()):+d} B over {len(delta)} names")
for n, d in sorted(delta.items(), key=lambda x: -abs(x[1]))[:60]:
    print(f"{d:+7d}  base {base[n]:6d}  other {other[n]:6d}  {n[:170]}")
PY
  ;;
esac
