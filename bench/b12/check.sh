#!/usr/bin/env bash
# B12 check for the catalog reader (plans/09-phase-2-work-order.md, A9;
# plans/06-size-and-perf.md §3 "How B6, B12, B13 are checked"; 05 §8).
#
# Builds the three no_std harnesses of this workspace for
# wasm32-unknown-unknown, twice: profile `wasm-release` (the 06 §3 size
# method: opt-level z, fat LTO, 1 CGU, panic=abort, strip) and `wasm-syms`
# (the same, keeping symbol names). Each goes through `wasm-opt -Oz`. Then:
#
#  1. Panic reachability. The harnesses' #[panic_handler] calls the import
#     `b12::b12_panic_reachable`; it is in the module only if some panic path
#     survived LTO + wasm-opt. It must be ABSENT from b12-reader (and
#     b12-base), and PRESENT in b12-control (a deliberate bounds check), which
#     proves the check can fail. Imports from any module other than `b12` are
#     undefined symbols (05 §8: mark imports with #[link(wasm_import_module)])
#     and fail too.
#  2. Symbols. twiggy over the non-stripped build, before and after wasm-opt:
#     no core::fmt / alloc::fmt / Formatter / Arguments / Debug / Display
#     symbols, no panic / bounds / overflow / capacity / alloc-error symbols.
#     b12-control (a deliberate `write!`) must show both kinds.
#  3. Data. No panic message text in the stripped optimised module.
#  4. Size. The reader's cost as the delta b12-reader − b12-base (same
#     scaffolding, an allocation kept alive), raw and gzip -9 — the reader's
#     share of B1 (plus the harness's walk, which a runtime has in its own form).
#
# Exit status: 0 when B12 holds, 1 when it does not (or the control shows the
# check is broken), 2 when a tool is missing. Report: target/b12/b12.txt and
# target/b12/size.tsv (under bench/b12/).
#
# Tools: cargo (rust-toolchain.toml, with the wasm32-unknown-unknown target),
# wasm-opt and wasm-dis (binaryen), twiggy, gzip.
set -euo pipefail
cd "$(dirname "$0")"

for tool in cargo wasm-opt wasm-dis twiggy gzip; do
  command -v "$tool" >/dev/null 2>&1 || { echo "b12: $tool not found on PATH" >&2; exit 2; }
done

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
TARGET=wasm32-unknown-unknown
OUT=target/b12
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CRATES=(base reader control)
PANIC_IMPORT='b12::b12_panic_reachable'
# twiggy demangles v0 names as `core[1a2b…]::fmt::…`; the mangled spellings
# (`4core3fmt`) are matched too in case a name is left mangled.
FMT_RE='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|4core3fmt|5alloc3fmt|fmt::Formatter|fmt::Arguments|Debug|Display'
PANIC_RE='panic|unwrap|expect|bounds|overflow|unreachable|capacity|handle_alloc_error|oom'
PANIC_TEXT_RE='panicked|out of bounds|called `|capacity overflow|attempt to |unwrap|overflow|slice index|byte index|memory allocation'

pkgs=(); for c in "${CRATES[@]}"; do pkgs+=(-p "b12-$c"); done
cargo build -q --target "$TARGET" --profile wasm-release "${pkgs[@]}"
cargo build -q --target "$TARGET" --profile wasm-syms "${pkgs[@]}"
mkdir -p "$OUT"

REPORT="$OUT/b12.txt"
: > "$REPORT"
say() { printf '%s\n' "$*" | tee -a "$REPORT"; }
fail=0
bad() { say "  FAIL: $*"; fail=1; }

say "B12 — catalog reader (mf2-catalog, no features), $(rustc --version)"
say "wasm-opt: $(wasm-opt --version); twiggy: $(twiggy --version)"

declare -A RAW GZ
for c in "${CRATES[@]}"; do
  rel="target/$TARGET/wasm-release/b12_$c.wasm"
  syms="target/$TARGET/wasm-syms/b12_$c.wasm"
  opt="$OUT/$c.opt.wasm"
  syms_opt="$OUT/$c.syms.opt.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$rel" -o "$opt"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$syms" -o "$syms_opt"
  RAW[$c]=$(stat -c %s "$opt")
  GZ[$c]=$(gzip -9 -n -c "$opt" | wc -c)

  say "== b12-$c"
  # 1. Imports of the stripped, optimised module (what ships).
  wasm-dis "$opt" | sed -nE 's/^ *\(import "([^"]*)" "([^"]*)".*/\1::\2/p' | sort > "$OUT/$c.imports"
  say "  imports: $(tr '\n' ' ' < "$OUT/$c.imports")"
  has_panic=no
  grep -qxF "$PANIC_IMPORT" "$OUT/$c.imports" && has_panic=yes
  say "  panic import $PANIC_IMPORT present after LTO + wasm-opt -Oz: $has_panic"
  foreign=$(grep -v '^b12::' "$OUT/$c.imports" || true)
  if [ -n "$foreign" ]; then
    bad "imports outside module b12 (undefined symbols): $(tr '\n' ' ' <<< "$foreign")"
  fi
  if [ "$c" = control ]; then
    [ "$has_panic" = yes ] || bad "the control's deliberate panic path left no import: the check is broken"
  else
    [ "$has_panic" = no ] || bad "a panic path survives in b12-$c"
  fi

  # 2. Symbols of the non-stripped build, before and after wasm-opt.
  for w in "$syms" "$syms_opt"; do
    csv="$OUT/$(basename "$w" .wasm).csv"
    twiggy top -n 1000000 --format csv "$w" > "$csv"
    names="$csv.names"
    # Name = the line without its four numeric columns (names may hold
    # commas); custom sections (`.debug_*`, `name`, …) are not code.
    tail -n +2 "$csv" | sed -E 's/(,[^,]*){4}$//' | grep -vE 'custom section|subsection' > "$names" || true
    n_fmt=$(grep -cE "$FMT_RE" "$names" || true)
    n_panic=$(grep -ciE "$PANIC_RE" "$names" || true)
    say "  $(basename "$w"): $(wc -l < "$names") items, fmt symbols $n_fmt, panic/alloc-failure symbols $n_panic"
    { grep -E "$FMT_RE" "$names"; grep -iE "$PANIC_RE" "$names"; } | sort -u | head -n 20 \
      | sed 's/^/      /' | tee -a "$REPORT" || true
    if [ "$c" = control ]; then
      [ "$n_fmt" -gt 0 ] && [ "$n_panic" -gt 0 ] \
        || bad "the control's deliberate fmt and panic code left no symbols: the check is broken"
    elif [ "$((n_fmt + n_panic))" -ne 0 ]; then
      bad "fmt or panic symbols in b12-$c ($(basename "$w"))"
    fi
  done

  # 3. Panic message text in the shipped module.
  n_text=$( { LC_ALL=C grep -a -o -iE "$PANIC_TEXT_RE" "$opt" || true; } | wc -l)
  say "  panic message strings in the stripped optimised module: $n_text"
  if [ "$c" != control ] && [ "$n_text" -ne 0 ]; then
    bad "panic message text in b12-$c"
  fi
done

# 4. Size: the reader as a delta against the base.
{
  printf 'harness\traw\tgz\tdelta_raw\tdelta_gz\n'
  printf 'base\t%d\t%d\t-\t-\n' "${RAW[base]}" "${GZ[base]}"
  printf 'reader\t%d\t%d\t%d\t%d\n' "${RAW[reader]}" "${GZ[reader]}" \
    $((RAW[reader] - RAW[base])) $((GZ[reader] - GZ[base]))
} > "$OUT/size.tsv"
say "== size (wasm-release, wasm-opt -Oz, gzip -9 -n; delta against b12-base)"
awk -F '\t' '{ printf "  %-8s %8s %8s %10s %10s\n", $1, $2, $3, $4, $5 }' "$OUT/size.tsv" | tee -a "$REPORT"
# Where the reader's bytes are (shallow code bytes after wasm-opt, by crate).
csv="$OUT/reader.syms.opt.csv"
awk -F, 'NR > 1 {
    n = $(NF - 3) + 0; name = $1; for (i = 2; i <= NF - 4; i++) name = name "," $i
    gsub(/"/, "", name)
    if (name ~ /section|subsection|headers|wasm magic/) next
    if (name ~ /b12_reader\[[0-9a-f]+\]::load$/) k = "Catalog::new (inlined into load)"
    else if (name ~ /mf2_catalog/) k = "mf2_catalog (other functions)"
    else if (name ~ /b12_reader|^run$/) k = "b12_reader walk (harness)"
    else if (name ~ /b12_harness/) k = "b12_harness (in the base too)"
    else if (name ~ /^<?core/) k = "core (from_utf8, iterators, ...)"
    else if (name ~ /^data segment/) k = "data segments"
    else k = "other (memcmp, allocator, imports, exports, ...)"
    sum[k] += n
  } END { for (k in sum) printf "  %6d B  %s\n", sum[k], k }' "$csv" | sort -rn | tee -a "$REPORT"
say "  largest reader items (twiggy top, optimised, names kept):"
twiggy top -n 24 "$OUT/reader.syms.opt.wasm" | grep -vE "custom section|subsection" | sed 's/^/    /' | tee -a "$REPORT"

if [ "$fail" -ne 0 ]; then
  say "B12: FAILED"
  exit 1
fi
say "B12: clean (no panic path, no core::fmt in the reader)"
