#!/usr/bin/env bash
# Item 1 (size) and the modules every other item loads.
#
#   bench/intl-probe/scripts/build.sh            # the working tree's crates/
#   INTL_PROBE_REV=3fc4735 bench/intl-probe/scripts/build.sh   # a commit's
#
# Per variant (base rust intl intl-loc intl-cu intl-codes, and rust-loc once
# wasm/Cargo.toml has that feature), in its own cargo invocation so no
# feature unifies: the 06 §3 method — profile `wasm-release` (opt-level z,
# fat LTO, 1 CGU, panic=abort, strip) → `wasm-bindgen --target web` →
# `wasm-opt -Oz` (bench/browser-no-fmt/check.sh's feature flags) → `gzip -9 -n`; raw and
# gz of the wasm and of the JS glue (probe.js + snippets/), each as a delta
# against `base`. The same with profile `wasm-syms` (names kept) for twiggy.
#
# Output: target/intl-probe/pkg/<variant>/ (what the page loads) and
# pkg/tree.txt, target/intl-probe/size.{tsv,md} (and size-<rev|tree>.*),
# twiggy-<variant>.{txt,csv}.
# Tools: cargo (rust-toolchain.toml + wasm32-unknown-unknown), wasm-bindgen
# 0.2.128 (= the library pin), wasm-opt (binaryen), twiggy, gzip.
source "$(dirname "$0")/common.sh"
for tool in cargo wasm-bindgen wasm-opt twiggy gzip; do
  command -v "$tool" >/dev/null 2>&1 || { echo "build.sh: $tool not found" >&2; exit 2; }
done
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
VARIANTS=(base rust intl intl-loc intl-cu intl-codes)
# `rust-loc` needs mf2-fn-number (Phase 4 A3; a snapshot of an older commit
# has a stub).
grep -q 'pub static PERCENT' "$SRC/crates/mf2-fn-number/src/lib.rs" 2>/dev/null && VARIANTS+=(rust-loc)
# `rust-cu` needs mf2-fn-number's :currency and :unit (A4), the `rt-intl*`
# variants the option as built (the runtime's `web-number-intl` feature).
grep -q 'pub static CURRENCY' "$SRC/crates/mf2-fn-number/src/measure.rs" 2>/dev/null && VARIANTS+=(rust-cu)
grep -q 'INTL_NUMBERS' "$SRC/crates/mf2-runtime/src/lib.rs" 2>/dev/null && VARIANTS+=(rt-intl rt-intl-loc rt-intl-cu)
# VARIANTS="base rust …" in the environment builds only those.
[ -n "${PROBE_VARIANTS:-}" ] && read -r -a VARIANTS <<< "$PROBE_VARIANTS"
TARGET=wasm32-unknown-unknown
cd "$PROBE"

gz() { gzip -9 -n -c "$@" | wc -c; }
{
  printf 'variant\twasm_raw\twasm_gz\tjs_raw\tjs_gz\n'
} > "$OUT/size.tsv"
for v in "${VARIANTS[@]}"; do
  for profile in wasm-release wasm-syms; do
    cargo build -q --target "$TARGET" --profile "$profile" -p intl-probe-wasm --features "$v"
    src="target/$TARGET/$profile/intl_probe_wasm.wasm"
    if [ "$profile" = wasm-release ]; then
      pkg="$OUT/pkg/$v"
      rm -rf "$pkg"
      wasm-bindgen --target web --out-dir "$pkg" --out-name probe "$src"
      wasm-opt -Oz "${FEATURES[@]}" "$pkg/probe_bg.wasm" -o "$pkg/probe_bg.wasm"
      wraw=$(stat -c %s "$pkg/probe_bg.wasm")
      wgz=$(gz "$pkg/probe_bg.wasm")
      # The JS glue: the main module and every snippet (inline_js), each
      # gzipped as served.
      jraw=0; jgz=0
      while IFS= read -r f; do
        jraw=$((jraw + $(stat -c %s "$f")))
        jgz=$((jgz + $(gz "$f")))
      done < <(find "$pkg" -name '*.js' | sort)
      printf '%s\t%d\t%d\t%d\t%d\n' "$v" "$wraw" "$wgz" "$jraw" "$jgz" >> "$OUT/size.tsv"
    else
      syms="$OUT/syms/$v"
      rm -rf "$syms"
      wasm-bindgen --target web --out-dir "$syms" --out-name probe "$src"
      wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$syms/probe_bg.wasm" -o "$syms/probe_bg.wasm"
      twiggy top -n 60 "$syms/probe_bg.wasm" > "$OUT/twiggy-$v.txt"
      twiggy top -n 1000000 --format csv "$syms/probe_bg.wasm" > "$OUT/twiggy-$v.csv"
    fi
  done
done

# The report: raw and gz, and deltas against base.
awk -F '\t' -v tree="$(tree_note)" -v tools="$(rustc --version); wasm-bindgen $(wasm-bindgen --version | cut -d' ' -f2); $(wasm-opt --version); $(gzip --version | head -1)" '
  NR == 1 { next }
  { v[NR] = $1; wr[NR] = $2; wg[NR] = $3; jr[NR] = $4; jg[NR] = $5; if ($1 == "base") b = NR; n = NR }
  END {
    print "Built from " tree "; " tools "."
    print ""
    print "| Variant | wasm raw | wasm gz | Δ wasm raw | **Δ wasm gz** | JS raw | JS gz | Δ JS raw | **Δ JS gz** | **Δ total gz** |"
    print "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    for (i = 2; i <= n; i++)
      printf "| `%s` | %d | %d | %+d | **%+d** | %d | %d | %+d | **%+d** | **%+d** |\n", v[i], wr[i], wg[i], wr[i]-wr[b], wg[i]-wg[b], jr[i], jg[i], jr[i]-jr[b], jg[i]-jg[b], (wg[i]-wg[b])+(jg[i]-jg[b])
  }' "$OUT/size.tsv" > "$OUT/size.md"
# Kept per tree (a later build replaces size.md and pkg/, which the page
# loads; pkg/tree.txt says which tree they are).
tag=tree
[ -n "${INTL_PROBE_REV:-}" ] && tag=$(git -C "$REPO" rev-parse --short "$INTL_PROBE_REV")
cp "$OUT/size.md" "$OUT/size-$tag.md"
cp "$OUT/size.tsv" "$OUT/size-$tag.tsv"
tree_note > "$OUT/pkg/tree.txt"
cat "$OUT/size.md"
