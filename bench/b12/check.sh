#!/usr/bin/env bash
# B12 check for the catalog reader (plans/09-phase-2-work-order.md, A9) and
# the runtime (plans/10-phase-3-work-order.md, A11), with B13 and the size of
# B1's runtime part (plans/06-size-and-perf.md §3 "How B6, B12, B13 are
# checked"; 05 §8).
#
# Builds the no_std harnesses of this workspace for
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
#  4. Size. Each harness as a delta against b12-base (same scaffolding, an
#     allocation kept alive), raw and gzip -9: the reader (plus its walk), the
#     runtime with every core function (B1's runtime part), the runtime with
#     :string only; the difference of the last two is the core numeric
#     semantics' share of B1 (≤ 10 KB gz).
#  5. B13. b12-runtime-nonum links no numeric handler: none of their symbols
#     (resolution, digit options, rounding, the plural evaluator) — and
#     b12-runtime, which uses them, shows them (the grep can fail).
#  6. The D15 A/B (plans/10 A5b): b12-runtime-fixed, the runtime with the
#     numeric code over fixed_decimal, built in its own cargo invocation (so
#     its feature does not reach the others) and reported only: its panic
#     paths are why the own buffer exists.
#  7. Phase 4 (plans/11 A11): B2 = b12-runtime-fn-number − b12-runtime
#     (mf2-fn-number on and used: the localized :number, :integer, :offset,
#     :percent and unannotated numbers) ≤ 3 KB gz; B1′ = b12-runtime-fn-number-
#     unused − b12-runtime (the crate linked, the registry the core's) = +0 B.
#     B3 = b12-runtime-fn-number-measure − b12-runtime-fn-number (:currency
#     and :unit too) ≤ 5.5 KB gz (restated by the owner, 2026-09-22). All are
#     B12-checked like the runtime.
#  8. The `intl` client option (owner decision 4; plans/03-runtime.md §2.7,
#     §5.3), built in their own cargo invocation (their `intl` features must
#     not reach the others): b12-runtime-intl (the core's numeric functions
#     over a stub number formatter) and b12-runtime-fn-number-intl (the whole
#     localized family) are B12-checked, and b12-runtime-intl must link none
#     of the Rust rounding, digit display or plural evaluator (B13's grep the
#     other way round: b12-runtime shows them); B1′ for `intl` =
#     b12-runtime-intl-unused (the features on, no number in the corpus) −
#     b12-runtime-nonum ≤ +0 B (it is below: a resolved number keeps its
#     digit plan, not the rounded digits, so every `Value` is smaller). The
#     other sizes are reported, not gated: the
#     formatter is a stub here, and the browser's (mf2-host-web's `Intl` glue
#     and its JavaScript) is measured by bench/intl-probe.
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
CRATES=(base reader runtime runtime-nonum runtime-fn-number runtime-fn-number-unused runtime-fn-number-measure control)
# The `intl` harnesses: one cargo invocation of their own.
INTL_CRATES=(runtime-intl runtime-fn-number-intl runtime-intl-unused)
PANIC_IMPORT='b12::b12_panic_reachable'
# twiggy demangles v0 names as `core[1a2b…]::fmt::…`; the mangled spellings
# (`4core3fmt`) are matched too in case a name is left mangled.
FMT_RE='core(\[[0-9a-f]+\])?::fmt|alloc(\[[0-9a-f]+\])?::fmt|4core3fmt|5alloc3fmt|fmt::Formatter|fmt::Arguments|Debug|Display'
PANIC_RE='panic|unwrap|expect|bounds|overflow|unreachable|capacity|handle_alloc_error|oom'
PANIC_TEXT_RE='panicked|out of bounds|called `|capacity overflow|attempt to |unwrap|overflow|slice index|byte index|memory allocation'

pkgs=(); for c in "${CRATES[@]}"; do pkgs+=(-p "b12-$c"); done
cargo build -q --target "$TARGET" --profile wasm-release "${pkgs[@]}"
cargo build -q --target "$TARGET" --profile wasm-syms "${pkgs[@]}"
# The A/B baseline alone: its `fixed-decimal` feature must not unify into the
# harnesses above.
cargo build -q --target "$TARGET" --profile wasm-release -p b12-runtime-fixed
cargo build -q --target "$TARGET" --profile wasm-syms -p b12-runtime-fixed
intl_pkgs=(); for c in "${INTL_CRATES[@]}"; do intl_pkgs+=(-p "b12-$c"); done
cargo build -q --target "$TARGET" --profile wasm-release "${intl_pkgs[@]}"
cargo build -q --target "$TARGET" --profile wasm-syms "${intl_pkgs[@]}"
mkdir -p "$OUT"

REPORT="$OUT/b12.txt"
: > "$REPORT"
say() { printf '%s\n' "$*" | tee -a "$REPORT"; }
fail=0
bad() { say "  FAIL: $*"; fail=1; }

say "B12 — catalog reader and runtime (mf2-catalog and mf2-runtime, no features), $(rustc --version)"
say "wasm-opt: $(wasm-opt --version); twiggy: $(twiggy --version)"

declare -A RAW GZ
for c in "${CRATES[@]}" runtime-fixed "${INTL_CRATES[@]}"; do
  file="b12_${c//-/_}"
  rel="target/$TARGET/wasm-release/$file.wasm"
  syms="target/$TARGET/wasm-syms/$file.wasm"
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
  elif [ "$c" = runtime-fixed ]; then
    say "  (the A/B baseline: reported, not gated)"
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
    elif [ "$c" = runtime-fixed ]; then
      :
    elif [ "$((n_fmt + n_panic))" -ne 0 ]; then
      bad "fmt or panic symbols in b12-$c ($(basename "$w"))"
    fi
  done

  # 3. Panic message text in the shipped module.
  n_text=$( { LC_ALL=C grep -a -o -iE "$PANIC_TEXT_RE" "$opt" || true; } | wc -l)
  say "  panic message strings in the stripped optimised module: $n_text"
  if [ "$c" != control ] && [ "$c" != runtime-fixed ] && [ "$n_text" -ne 0 ]; then
    bad "panic message text in b12-$c"
  fi
done

# 5. B13: no numeric handler in the runtime built without one.
NUM_RE='NumberFunction|number::(resolve|display|raw_fixed|raw_precision|matches|exact_matches|operands|options::)|mf2_runtime(\[[0-9a-f]+\])?::plural::|Decimal>::(round|add)'
say "== B13 (closed world): numeric-handler symbols"
for c in runtime runtime-nonum; do
  names="$OUT/$c.syms.opt.csv.names"
  n_num=$(grep -cE "$NUM_RE" "$names" || true)
  say "  b12-$c: $n_num"
  if [ "$c" = runtime ]; then
    [ "$n_num" -gt 0 ] || bad "b12-runtime shows no numeric-handler symbol: the B13 grep is broken"
  else
    [ "$n_num" -eq 0 ] || bad "b12-runtime-nonum links numeric handlers (B13)"
  fi
done

# 8. The `intl` option links no Rust rounding, display or plural evaluator.
RUST_DIGITS_RE='number::display::|Decimal>::round|plural::select|OperandsBuilder'
say "== intl (8): Rust rounding, display and plural-evaluator symbols"
for c in runtime runtime-intl; do
  names="$OUT/$c.syms.opt.csv.names"
  n_rust=$(grep -cE "$RUST_DIGITS_RE" "$names" || true)
  say "  b12-$c: $n_rust"
  if [ "$c" = runtime ]; then
    [ "$n_rust" -gt 0 ] || bad "b12-runtime shows no Rust rounding symbol: the intl grep is broken"
  else
    [ "$n_rust" -eq 0 ] || bad "b12-runtime-intl links the Rust rounding, display or plural evaluator"
  fi
done

# 4. Size: each harness as a delta against the base.
{
  printf 'harness\traw\tgz\tdelta_raw\tdelta_gz\n'
  printf 'base\t%d\t%d\t-\t-\n' "${RAW[base]}" "${GZ[base]}"
  for c in reader runtime runtime-nonum runtime-fixed runtime-fn-number runtime-fn-number-unused runtime-fn-number-measure "${INTL_CRATES[@]}"; do
    printf '%s\t%d\t%d\t%d\t%d\n' "$c" "${RAW[$c]}" "${GZ[$c]}" \
      $((RAW[$c] - RAW[base])) $((GZ[$c] - GZ[base]))
  done
  printf 'numbers (runtime - runtime-nonum)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime] - RAW[runtime-nonum])) $((GZ[runtime] - GZ[runtime-nonum]))
  printf 'numbers over fixed_decimal (runtime-fixed - runtime-nonum)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fixed] - RAW[runtime-nonum])) $((GZ[runtime-fixed] - GZ[runtime-nonum]))
  printf 'B2: fn-number on and used (runtime-fn-number - runtime)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number] - RAW[runtime])) $((GZ[runtime-fn-number] - GZ[runtime]))
  printf "B1': fn-number on, unused (runtime-fn-number-unused - runtime)\t-\t-\t%d\t%d\n" \
    $((RAW[runtime-fn-number-unused] - RAW[runtime])) $((GZ[runtime-fn-number-unused] - GZ[runtime]))
  printf 'B3: + :currency, :unit (runtime-fn-number-measure - runtime-fn-number)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number-measure] - RAW[runtime-fn-number])) $((GZ[runtime-fn-number-measure] - GZ[runtime-fn-number]))
  printf 'intl core, stub formatter (runtime-intl - runtime)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-intl] - RAW[runtime])) $((GZ[runtime-intl] - GZ[runtime]))
  printf 'intl + fn-number (runtime-fn-number-intl - runtime-fn-number-measure)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number-intl] - RAW[runtime-fn-number-measure])) $((GZ[runtime-fn-number-intl] - GZ[runtime-fn-number-measure]))
  printf "B1': intl on, unused (runtime-intl-unused - runtime-nonum)\t-\t-\t%d\t%d\n" \
    $((RAW[runtime-intl-unused] - RAW[runtime-nonum])) $((GZ[runtime-intl-unused] - GZ[runtime-nonum]))
} > "$OUT/size.tsv"
# B3 ≤ 5.5 KB gz more (plans/06-size-and-perf.md §3; restated 2026-09-22).
b3=$((GZ[runtime-fn-number-measure] - GZ[runtime-fn-number]))
[ "$b3" -le 5632 ] || bad "B3: :currency + :unit cost $b3 B gz (> 5,632)"
# B2 ≤ 3 KB gz; B1′ = +0 B (plans/06-size-and-perf.md §3).
b2=$((GZ[runtime-fn-number] - GZ[runtime]))
[ "$b2" -le 3072 ] || bad "B2: fn-number on and used costs $b2 B gz (> 3,072)"
[ "${RAW[runtime-intl-unused]}" -le "${RAW[runtime-nonum]}" ] \
  || bad "B1': intl on but unused costs more than +0 B (raw ${RAW[runtime-intl-unused]} vs ${RAW[runtime-nonum]})"
[ "${RAW[runtime-fn-number-unused]}" -eq "${RAW[runtime]}" ] \
  || bad "B1': fn-number on but unused is not +0 B (raw ${RAW[runtime-fn-number-unused]} vs ${RAW[runtime]})"
say "== size (wasm-release, wasm-opt -Oz, gzip -9 -n; delta against b12-base)"
awk -F '\t' '{ printf "  %-72s %8s %8s %10s %10s\n", $1, $2, $3, $4, $5 }' "$OUT/size.tsv" | tee -a "$REPORT"
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
say "B12: clean (no panic path, no core::fmt in the reader or the runtime); B13: shown"
