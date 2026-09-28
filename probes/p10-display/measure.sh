#!/usr/bin/env bash
# A5: run the four measurements and save the results under target/a5/$1/.
set -uo pipefail
label="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
out="target/a5/$label"
rm -rf "$out"
mkdir -p "$out"
export CARGO_BUILD_JOBS=2
echo "== $label at $(git log -1 --format=%h) $(date -Is)" | tee "$out/meta.txt"
git status --short | grep -v '^??' >> "$out/meta.txt" || true
run() {
  local name="$1"
  shift
  echo "-- $name start $(date -Is) load: $(cut -d' ' -f1-3 /proc/loadavg)" | tee -a "$out/meta.txt"
  "$@" > "$out/$name.stdout" 2> "$out/$name.stderr"
  local rc=$?
  echo "-- $name exit $rc end $(date -Is)" | tee -a "$out/meta.txt"
  return 0
}
keep_wasm() {
  local kind="$1"
  for d in target/"$kind"/wl-*; do
    for p in "$d"/pkg-*; do
      [ -d "$p" ] || continue
      local dest="$out/wasm/$kind/$(basename "$d")/$(basename "$p")"
      mkdir -p "$dest"
      cp "$p"/*.wasm "$dest/" 2>/dev/null || true
    done
  done
}
run size cargo xtask size
keep_wasm size
run b5-view cargo xtask b5 --view
keep_wasm b5
run b12-generated cargo xtask b12-generated
mkdir -p "$out/wasm/b12-generated"
find target/b12-generated -path '*wasm32-unknown-unknown/wasm-release/mf2-i18n-client.wasm' -exec cp {} "$out/wasm/b12-generated/" \; 2>/dev/null || true
run b12-check bash bench/b12/check.sh
cp bench/b12/target/b12/b12.txt bench/b12/target/b12/size.tsv "$out/" 2>/dev/null || true
echo "== $label done $(date -Is)" | tee -a "$out/meta.txt"
