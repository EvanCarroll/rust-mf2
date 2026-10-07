#!/usr/bin/env bash
# B12 check for the catalog reader and
# the runtime, with B13 and the size of
# B1's runtime part.
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
#     allocation kept alive), raw and brotli -q 11: the reader (plus its walk), the
#     runtime with every core function (B1's runtime part), the runtime with
#     :string only; the difference of the last two is the core numeric
#     semantics' share of B1 (≤ 9 KB br).
#  5. B13. b12-runtime-nonum links no numeric handler: none of their symbols
#     (resolution, digit options, rounding, the plural evaluator) — and
#     b12-runtime, which uses them, shows them (the grep can fail).
#  6. The D15 A/B: b12-runtime-fixed, the runtime with the
#     numeric code over fixed_decimal, built in its own cargo invocation (so
#     its feature does not reach the others) and reported only: its panic
#     paths are why the own buffer exists.
#  7. Phase 4: B2 = b12-runtime-fn-number − b12-runtime
#     (mf2-fn-number on and used: the localized :number, :integer, :offset,
#     :percent and unannotated numbers) ≤ 2.75 KB br; B1′ = b12-runtime-fn-number-
#     unused − b12-runtime (the crate linked, the registry the core's) = +0 B.
#     B3 = b12-runtime-fn-number-measure − b12-runtime-fn-number (:currency
#     and :unit too) ≤ 5 KB br (5.5 KB gzip, restated by the owner 2026-09-22,
#     scaled when the wasm figures moved to brotli, 2026-10-05). All are
#     B12-checked like the runtime.
#  8. The `intl` client option (owner decision 4), built in their own cargo invocation (their `intl` features must
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
#  9. Dates, over b12-dates-walk (the runtime walk with
#     date/time arguments and the context's zone from the host), against
#     b12-dates-base (that walk, the core registry):
#     * b12-dates-semantics: the date semantics every backend needs (a
#       backend writing one byte of the plan) — reported beside B4's
#       note (≤ 3,190 B br, 3,584 gzip scaled; measured 3,006);
#       b12-dates-neutral: with the neutral stub backend.
#       Both B12-gated.
#     * B4 icu: b12-dates-icu-{greg,any}-{nozones,zones} (ICU4X over
#       the catalog's icu.blob): Gregorian with zone styles ≤ 85 KB br, any
#       calendar with zone styles ≤ 94 KB br; the no-zone variants reported.
#       B12 reported, not gated: ICU4X keeps its own core::fmt and panic
#       paths, the feature's documented cost (06 B4).
#     * B4 intl: b12-dates-intl (Intl.DateTimeFormat through
#       mf2-host-web's INTL_HOST) − b12-dates-web-base (the same walk on
#       mf2-host-web's HOST), both through wasm-bindgen: wasm ≤ 5.5 KB br and
#       JS glue ≤ 1 KB br. mf2-host-web's own glue (js-sys) has a panic
#       path, reported with the web base; b12-dates-intl must add no fmt or
#       panic symbol, panic import or panic text to it.
#     * B13: b12-dates-unused (datetime linked with both backend
#       features, unused), b12-dates-base and b12-runtime link no date
#       symbol; b12-dates-semantics shows them (the grep can fail).
#     * B1′: b12-dates-unused = b12-runtime, +0 B; and b12-dates-web-base,
#       built with mf2-host-web's date features on, loads no date glue (its
#       JS imports no snippet) and equals b12-dates-web-plain, its source
#       built alone without them (wasm and JS).
#
# Exit status: 0 when B12 holds, 1 when it does not (or the control shows the
# check is broken), 2 when a tool is missing. Report: target/b12/b12.txt and
# target/b12/size.tsv (under bench/browser-no-fmt/).
#
# Tools: cargo (rust-toolchain.toml, with the wasm32-unknown-unknown target),
# wasm-opt and wasm-dis (binaryen), twiggy, brotli, and the wasm-bindgen CLI
# of the version the harnesses resolve (0.2.128).
set -euo pipefail
cd "$(dirname "$0")"

for tool in cargo wasm-opt wasm-dis twiggy brotli wasm-bindgen; do
  command -v "$tool" >/dev/null 2>&1 || { echo "b12: $tool not found on PATH" >&2; exit 2; }
done

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
TARGET=wasm32-unknown-unknown
OUT=target/browser-no-fmt
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
CRATES=(base reader runtime runtime-nonum runtime-fn-number runtime-fn-number-unused runtime-fn-number-measure control
  dates-base dates-semantics dates-neutral dates-unused
  dates-icu-greg-nozones dates-icu-greg-zones dates-icu-any-nozones dates-icu-any-zones
  dates-web-base dates-intl)
# Reported, not gated for B12 (see 6. and 9. above).
REPORTED=" runtime-fixed dates-icu-greg-nozones dates-icu-greg-zones dates-icu-any-nozones dates-icu-any-zones "
# Built through wasm-bindgen (their imports are the JS glue's).
WEB=" dates-web-base dates-web-plain dates-intl "
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
# B1′ for mf2-host-web's date features: the web base without them, alone.
cargo build -q --target "$TARGET" --profile wasm-release -p b12-dates-web-plain
cargo build -q --target "$TARGET" --profile wasm-syms -p b12-dates-web-plain
mkdir -p "$OUT"

REPORT="$OUT/browser-no-fmt.txt"
: > "$REPORT"
say() { printf '%s\n' "$*" | tee -a "$REPORT"; }
fail=0
bad() { say "  FAIL: $*"; fail=1; }

say "B12 — catalog reader and runtime (mf2-catalog and mf2-runtime, no features), $(rustc --version)"
say "wasm-opt: $(wasm-opt --version); twiggy: $(twiggy --version)"

declare -A RAW BR JS TEXT
for c in "${CRATES[@]}" runtime-fixed dates-web-plain "${INTL_CRATES[@]}"; do
  file="b12_${c//-/_}"
  rel="target/$TARGET/wasm-release/$file.wasm"
  syms="target/$TARGET/wasm-syms/$file.wasm"
  if [[ "$WEB" == *" $c "* ]]; then
    # What ships is wasm-bindgen's output: the module and its JS glue.
    # One output name for all (`h`), so that no harness's name is in the
    # bytes compared (the glue's module name is in every import).
    rm -rf "$OUT/web/$c" "$OUT/web-syms/$c"
    wasm-bindgen --target web --out-name h --out-dir "$OUT/web/$c" "$rel"
    wasm-bindgen --target web --out-name h --out-dir "$OUT/web-syms/$c" "$syms"
    rel="$OUT/web/$c/h_bg.wasm"
    syms="$OUT/web-syms/$c/h_bg.wasm"
    # The JS a page loads: the glue, and each snippet the glue imports.
    js=$(brotli -q 11 --lgwin=22 -c "$OUT/web/$c/h.js" | wc -c)
    for snippet in $(sed -nE "s/^import .* from '\.\/(snippets\/[^']*)';$/\1/p" "$OUT/web/$c/h.js"); do
      js=$((js + $(brotli -q 11 --lgwin=22 -c "$OUT/web/$c/$snippet" | wc -c)))
    done
    JS[$c]=$js
  fi
  opt="$OUT/$c.opt.wasm"
  syms_opt="$OUT/$c.syms.opt.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$rel" -o "$opt"
  wasm-opt -Oz --debuginfo "${FEATURES[@]}" "$syms" -o "$syms_opt"
  RAW[$c]=$(stat -c %s "$opt")
  BR[$c]=$(brotli -q 11 --lgwin=22 -c "$opt" | wc -c)

  say "== b12-$c"
  # 1. Imports of the stripped, optimised module (what ships).
  wasm-dis "$opt" | sed -nE 's/^ *\(import "([^"]*)" "([^"]*)".*/\1::\2/p' | sort > "$OUT/$c.imports"
  say "  imports: $(tr '\n' ' ' < "$OUT/$c.imports")"
  has_panic=no
  grep -qxF "$PANIC_IMPORT" "$OUT/$c.imports" && has_panic=yes
  say "  panic import $PANIC_IMPORT present after LTO + wasm-opt -Oz: $has_panic"
  foreign=$(grep -vE "^b12::|^\./h_bg\.js::" "$OUT/$c.imports" || true)
  if [[ "$WEB" != *" $c "* ]]; then
    foreign=$(grep -v '^b12::' "$OUT/$c.imports" || true)
  fi
  if [ -n "$foreign" ]; then
    bad "imports outside module b12 (undefined symbols): $(tr '\n' ' ' <<< "$foreign")"
  fi
  if [ "$c" = control ]; then
    [ "$has_panic" = yes ] || bad "the control's deliberate panic path left no import: the check is broken"
  elif [ "$c" = runtime-fixed ]; then
    say "  (the A/B baseline: reported, not gated)"
  elif [[ "$REPORTED" == *" $c "* ]]; then
    say "  (ICU4X: reported, not gated)"
  elif [ "$c" = dates-web-base ] || [ "$c" = dates-web-plain ]; then
    say "  (mf2-host-web's own glue: reported; b12-dates-intl may add nothing to it)"
  elif [ "$c" = dates-intl ]; then
    base_panic=no
    grep -qxF "$PANIC_IMPORT" "$OUT/dates-web-base.imports" && base_panic=yes
    [ "$has_panic" = "$base_panic" ] || bad "b12-dates-intl adds a panic path to the web base"
  else
    [ "$has_panic" = no ] || bad "a panic path survives in b12-$c"
  fi

  # 2. Symbols of the non-stripped build, before and after wasm-opt.
  for w in "$syms" "$syms_opt"; do
    csv="$OUT/$(basename "$w" .wasm).csv"
    if [[ "$WEB" == *" $c "* ]] && [ "$w" = "$syms" ]; then
      csv="$OUT/$c.syms.csv"
    fi
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
    { grep -E "$FMT_RE" "$names"; grep -iE "$PANIC_RE" "$names"; } | sort -u > "$names.bad" || true
    if [ "$c" = control ]; then
      [ "$n_fmt" -gt 0 ] && [ "$n_panic" -gt 0 ] \
        || bad "the control's deliberate fmt and panic code left no symbols: the check is broken"
    elif [ "$c" = runtime-fixed ] || [[ "$REPORTED" == *" $c "* ]] || [ "$c" = dates-web-base ] \
      || [ "$c" = dates-web-plain ]; then
      :
    elif [ "$c" = dates-intl ]; then
      # Against the web base's list of the same stage (built alike).
      base_list="${csv/dates-intl/dates-web-base}.names.bad"
      extra=$(comm -23 "$names.bad" "$base_list" || true)
      [ -z "$extra" ] || bad "fmt or panic symbols b12-dates-intl adds to the web base: $(head -n 5 <<< "$extra" | tr '\n' ' ')"
    elif [ "$((n_fmt + n_panic))" -ne 0 ]; then
      bad "fmt or panic symbols in b12-$c ($(basename "$w"))"
    fi
  done

  # 3. Panic message text in the shipped module.
  n_text=$( { LC_ALL=C grep -a -o -iE "$PANIC_TEXT_RE" "$opt" || true; } | wc -l)
  TEXT[$c]=$n_text
  say "  panic message strings in the stripped optimised module: $n_text"
  if [ "$c" = dates-intl ]; then
    [ "$n_text" -le "${TEXT[dates-web-base]}" ] || bad "panic message text b12-dates-intl adds to the web base"
  elif [ "$c" != control ] && [ "$c" != runtime-fixed ] && [[ "$REPORTED" != *" $c "* ]] \
    && [ "$c" != dates-web-base ] && [ "$c" != dates-web-plain ] && [ "$n_text" -ne 0 ]; then
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
# 9. B13 for dates: no date code where no date function is used.
DATE_RE='mf2_fn_datetime|icu_datetime|icu_calendar|icu_time|icu_provider|icu_decimal'
say "== B13 (closed world): date symbols"
for c in runtime dates-base dates-unused dates-semantics; do
  names="$OUT/$c.syms.opt.csv.names"
  n_date=$(grep -cE "$DATE_RE" "$names" || true)
  say "  b12-$c: $n_date"
  if [ "$c" = dates-semantics ]; then
    [ "$n_date" -gt 0 ] || bad "b12-dates-semantics shows no date symbol: the B13 grep is broken"
  else
    [ "$n_date" -eq 0 ] || bad "b12-$c links date code (B13)"
  fi
done
# B1′ for mf2-host-web's date features: the web base (HOST) loads no snippet,
# and is the size it is without them.
if grep -qE "^import .* from '\./snippets/" "$OUT/web/dates-web-base/h.js"; then
  bad "B1': mf2-host-web's HOST loads date glue with its date features on"
else
  say "  b12-dates-web-base (HOST, date features on): its JS imports no snippet"
fi
if [ "${RAW[dates-web-base]}" -eq "${RAW[dates-web-plain]}" ] && [ "${JS[dates-web-base]}" -eq "${JS[dates-web-plain]}" ]; then
  say "  b12-dates-web-base = b12-dates-web-plain (without the features): ${RAW[dates-web-base]} B wasm, ${JS[dates-web-base]} B br JS"
else
  bad "B1': mf2-host-web's date features cost HOST $((RAW[dates-web-base] - RAW[dates-web-plain])) B wasm, $((JS[dates-web-base] - JS[dates-web-plain])) B br JS"
fi

# 4. Size: each harness as a delta against the base.
{
  printf 'harness\traw\tbr\tdelta_raw\tdelta_br\n'
  printf 'base\t%d\t%d\t-\t-\n' "${RAW[base]}" "${BR[base]}"
  for c in reader runtime runtime-nonum runtime-fixed runtime-fn-number runtime-fn-number-unused runtime-fn-number-measure "${INTL_CRATES[@]}"; do
    printf '%s\t%d\t%d\t%d\t%d\n' "$c" "${RAW[$c]}" "${BR[$c]}" \
      $((RAW[$c] - RAW[base])) $((BR[$c] - BR[base]))
  done
  printf 'numbers (runtime - runtime-nonum)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime] - RAW[runtime-nonum])) $((BR[runtime] - BR[runtime-nonum]))
  printf 'numbers over fixed_decimal (runtime-fixed - runtime-nonum)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fixed] - RAW[runtime-nonum])) $((BR[runtime-fixed] - BR[runtime-nonum]))
  printf 'B2: fn-number on and used (runtime-fn-number - runtime)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number] - RAW[runtime])) $((BR[runtime-fn-number] - BR[runtime]))
  printf "B1': fn-number on, unused (runtime-fn-number-unused - runtime)\t-\t-\t%d\t%d\n" \
    $((RAW[runtime-fn-number-unused] - RAW[runtime])) $((BR[runtime-fn-number-unused] - BR[runtime]))
  printf 'B3: + :currency, :unit (runtime-fn-number-measure - runtime-fn-number)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number-measure] - RAW[runtime-fn-number])) $((BR[runtime-fn-number-measure] - BR[runtime-fn-number]))
  for c in dates-base dates-semantics dates-neutral dates-unused dates-icu-greg-nozones dates-icu-greg-zones \
    dates-icu-any-nozones dates-icu-any-zones dates-web-base dates-web-plain dates-intl; do
    printf '%s\t%d\t%d\t%d\t%d\n' "$c" "${RAW[$c]}" "${BR[$c]}" \
      $((RAW[$c] - RAW[base])) $((BR[$c] - BR[base]))
  done
  printf 'date semantics (dates-semantics - dates-base)\t-\t-\t%d\t%d\n' \
    $((RAW[dates-semantics] - RAW[dates-base])) $((BR[dates-semantics] - BR[dates-base]))
  printf 'semantics + neutral backend (dates-neutral - dates-base)\t-\t-\t%d\t%d\n' \
    $((RAW[dates-neutral] - RAW[dates-base])) $((BR[dates-neutral] - BR[dates-base]))
  for c in dates-icu-greg-nozones dates-icu-greg-zones dates-icu-any-nozones dates-icu-any-zones; do
    printf 'B4 icu: %s - dates-base\t-\t-\t%d\t%d\n' "${c#dates-icu-}" \
      $((RAW[$c] - RAW[dates-base])) $((BR[$c] - BR[dates-base]))
  done
  printf 'B4 intl: wasm (dates-intl - dates-web-base)\t-\t-\t%d\t%d\n' \
    $((RAW[dates-intl] - RAW[dates-web-base])) $((BR[dates-intl] - BR[dates-web-base]))
  printf 'B4 intl: JS glue br (dates-intl %d - dates-web-base %d)\t-\t-\t-\t%d\n' \
    "${JS[dates-intl]}" "${JS[dates-web-base]}" $((JS[dates-intl] - JS[dates-web-base]))
  printf "B1': datetime on, unused (dates-unused - runtime)\t-\t-\t%d\t%d\n" \
    $((RAW[dates-unused] - RAW[runtime])) $((BR[dates-unused] - BR[runtime]))
  printf 'intl core, stub formatter (runtime-intl - runtime)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-intl] - RAW[runtime])) $((BR[runtime-intl] - BR[runtime]))
  printf 'intl + fn-number (runtime-fn-number-intl - runtime-fn-number-measure)\t-\t-\t%d\t%d\n' \
    $((RAW[runtime-fn-number-intl] - RAW[runtime-fn-number-measure])) $((BR[runtime-fn-number-intl] - BR[runtime-fn-number-measure]))
  printf "B1': intl on, unused (runtime-intl-unused - runtime-nonum)\t-\t-\t%d\t%d\n" \
    $((RAW[runtime-intl-unused] - RAW[runtime-nonum])) $((BR[runtime-intl-unused] - BR[runtime-nonum]))
} > "$OUT/size.tsv"
# B4: intl ≤ 5.5 KB br wasm + ≤ 1 KB br
# JS; icu ≤ 85 KB br Gregorian, ≤ 94 KB br any calendar (with zone
# styles, the widest of each); B1′ = +0 B for datetime on but unused.
b4_intl=$((BR[dates-intl] - BR[dates-web-base]))
[ "$b4_intl" -le 5632 ] || bad "B4: intl costs $b4_intl B br of wasm (> 5,632)"
b4_js=$((JS[dates-intl] - JS[dates-web-base]))
[ "$b4_js" -le 1024 ] || bad "B4: intl costs $b4_js B br of JS (> 1,024)"
b4_greg=$((BR[dates-icu-greg-zones] - BR[dates-base]))
[ "$b4_greg" -le 87040 ] || bad "B4: icu, Gregorian, costs $b4_greg B br (> 87,040)"
b4_any=$((BR[dates-icu-any-zones] - BR[dates-base]))
[ "$b4_any" -le 96256 ] || bad "B4: icu, any calendar, costs $b4_any B br (> 96,256)"
[ "${RAW[dates-unused]}" -eq "${RAW[runtime]}" ] \
  || bad "B1': datetime on but unused is not +0 B (raw ${RAW[dates-unused]} vs ${RAW[runtime]})"
# B3 ≤ 5 KB br more (5.5 KB gzip, restated 2026-09-22).
b3=$((BR[runtime-fn-number-measure] - BR[runtime-fn-number]))
[ "$b3" -le 5120 ] || bad "B3: :currency + :unit cost $b3 B br (> 5,120)"
# B2 ≤ 2.75 KB br; B1′ = +0 B.
b2=$((BR[runtime-fn-number] - BR[runtime]))
[ "$b2" -le 2816 ] || bad "B2: fn-number on and used costs $b2 B br (> 2,816)"
[ "${RAW[runtime-intl-unused]}" -le "${RAW[runtime-nonum]}" ] \
  || bad "B1': intl on but unused costs more than +0 B (raw ${RAW[runtime-intl-unused]} vs ${RAW[runtime-nonum]})"
[ "${RAW[runtime-fn-number-unused]}" -eq "${RAW[runtime]}" ] \
  || bad "B1': fn-number on but unused is not +0 B (raw ${RAW[runtime-fn-number-unused]} vs ${RAW[runtime]})"
say "== size (wasm-release, wasm-opt -Oz, brotli -q 11 --lgwin=22; delta against b12-base)"
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
say "B12: clean (no panic path, no core::fmt in the reader, the runtime, the numeric and the date functions); B13: shown"
