#!/usr/bin/env bash
# Phase 10: the project's whole check suite in one quiet run — `cargo xtask ci`
# first, then the web sizes and the demos' shipped files, the xtask checks, the
# browser checks on both Leptos lines, the size budgets and the TUI gate — one
# at a time, carrying on after a failure. The checks are the existing commands
# and scripts; this only runs them and reads their figures.
#
#   bash tools/checks/run.sh LABEL [--against EARLIER] [--only CHECK,...] [--summarize]
#
# Each check's output: target/p10-checks/LABEL/CHECK.log. stdout: one line per
# check, then the table (also summary.tsv). Exits 1 if any check failed.
# `--against` alternates the TUI gate with EARLIER's kept binaries and counts
# the demo files that changed; `--summarize` only rebuilds the table from the
# logs. `compare.sh` judges two runs; see README.md.
set -u
cd "$(dirname "$0")/../.."
all=(ci sizes demos docs docs-rs codegen-matrix scenarios msrv leptos-0-8 churn l6-web l7-web
     e2e-0-9 e2e-0-8 browser-no-fmt browser-pay-for-use conformance-report api refusals tui-allocs-vs-pseudotrippy native-no-heavy-crates)
usage='usage: run.sh LABEL [--against EARLIER] [--only CHECK,...] [--summarize]'
label=${1:?$usage}; shift
against= only= summarize=
while [ $# -gt 0 ]; do
  case $1 in
    --against) against=${2:?$usage}; shift 2 ;;
    --only) only=${2:?$usage}; shift 2 ;;
    --summarize) summarize=1; shift ;;
    *) echo "$usage" >&2; exit 2 ;;
  esac
done
for c in ${only//,/ }; do
  [[ " ${all[*]} " == *" $c "* ]] || { echo "unknown check $c; the checks: ${all[*]}" >&2; exit 2; }
done
if [ -n "$against" ] && [ ! -d "target/p10-checks/$against" ]; then
  echo "no run labelled $against in target/p10-checks/" >&2; exit 2
fi
out=target/p10-checks/$label
mkdir -p "$out"
if [ -n "$against" ]; then echo "$against" >"$out/against"; fi

# The checks. measure.sh and e2e.sh set CARGO_BUILD_JOBS=2 for cargo-leptos
# themselves; everything else builds with 3.
exec_check() {
  case $1 in
    ci) CARGO_BUILD_JOBS=3 cargo xtask ci ;;
    sizes)
      CARGO_BUILD_JOBS=3 bash tools/checks/measure.sh "$label"
      local l=target/p10-b2/logs/$label
      echo "== $l/size-report.md"; cat "$l/size-report.md"
      echo "== $l/b5v.log"; tail -12 "$l/b5v.log"
      echo "== $l/catalog-size.log"; cat "$l/catalog-size.log" ;;
    demos)
      bash tools/checks/demo-hashes.sh "${against:-$label}" "$label" &&
        cp "target/p10-b2/logs/hashes/$label.sha256" "$out/demo-files.sha256" ;;
    docs|docs-rs|codegen-matrix|scenarios|msrv|leptos-0-8|churn|l6-web|l7-web|browser-pay-for-use|refusals|native-no-heavy-crates)
      CARGO_BUILD_JOBS=3 cargo xtask "$1" ;;
    e2e-0-9) bash tools/checks/e2e.sh . "$label-leptos-0-9" ;;
    e2e-0-8)
      CARGO_BUILD_JOBS=3 python3 tools/checks/demos-0-8.py p10-checks &&
        bash tools/checks/e2e.sh target/a7-demo-0-8/p10-checks "$label-leptos-0-8" ;;
    browser-no-fmt) CARGO_BUILD_JOBS=3 bash bench/browser-no-fmt/check.sh ;;
    conformance-report)
      # The xtask has no --check: it checks the ledger and rewrites REPORT.md
      # and COVERAGE.md, which must come out as they were.
      local before; before=$(sha256sum conformance/REPORT.md conformance/COVERAGE.md)
      CARGO_BUILD_JOBS=3 cargo xtask conformance-report || return
      [ "$(sha256sum conformance/REPORT.md conformance/COVERAGE.md)" = "$before" ] ||
        { echo "run.sh: conformance/REPORT.md or COVERAGE.md changed"; return 1; } ;;
    api) CARGO_BUILD_JOBS=3 cargo xtask api --check ;;
    tui-allocs-vs-pseudotrippy)
      CARGO_BUILD_JOBS=3 cargo xtask tui-allocs-vs-pseudotrippy --save-baseline "$out/tui-baseline" \
        ${against:+--baseline "target/p10-checks/$against/tui-baseline"} ;;
  esac
}

# Demo files whose hashes differ between two hash lists, but for demo-ssr's
# __wasm_split loader, whose hash varies between identical builds.
changed_files() {
  awk '$2 ~ /__wasm_split/ { next } FILENAME == ARGV[1] { h[$2] = $1; next }
       { if (h[$2] != $1) print $2; delete h[$2] } END { for (p in h) print p }' "$1" "$2"
}

# A check's key figures, as KEY=VALUE words, read from its log.
figures() {
  local c=$1 f=$out/$1.log
  [ -f "$f" ] || return 0
  awk '/^test result:/ { t = 1; for (i = 1; i < NF; i++) { if ($(i+1) ~ /^passed/) p += $i; if ($(i+1) ~ /^failed/) x += $i } }
       / assertions passed/ { for (i = 1; i < NF; i++) if ($(i+1) == "assertions" && split($i, n, "/") == 2) { s = 1; ap += n[1]; at += n[2] } }
       END { if (t) printf "tests=%d ", p; if (x) printf "tests_failed=%d ", x; if (s) printf "asserts=%d/%d ", ap, at }' "$f"
  case $c in
    sizes) awk -F'|' '/^\| B1, fixed/ { b1 = $3 } /^\| B5, per site/ { b5 = $3 } /^\| whole app/ { app = $3 }
             /^per call site: / { split($0, w, " "); b5v = w[4] }
             /^\| [^|]* \| B7 brotli/ { l = $2; gsub(/ /, "", l); v = $4; gsub(/[^0-9]/, "", v); b7 = b7 " b7." l "=" v }
             END { gsub(/[^0-9.]/, "", b1); gsub(/[^0-9.]/, "", b5); gsub(/[^0-9.]/, "", app)
                   printf "b1=%s b5=%s app=%s b5v=%s%s", b1, b5, app, b5v, b7 }' "$f" ;;
    demos)
      local n=0 prev
      [ -f "$out/demo-files.sha256" ] && n=$(wc -l <"$out/demo-files.sha256")
      prev=target/p10-checks/$(cat "$out/against" 2>/dev/null)/demo-files.sha256
      if [ -s "$out/against" ] && [ -f "$prev" ] && [ "$n" != 0 ]; then
        echo "files=$n changed=$(changed_files "$prev" "$out/demo-files.sha256" | wc -l)"
      else echo "files=$n"; fi ;;
    docs-rs) sed -n 's/^docs-rs: \([0-9]*\) crates documented.*/crates=\1 /p' "$f" ;;
    codegen-matrix) sed -n 's/^codegen-matrix: .*(\([0-9]*\) B)$/rlib=\1 /p' "$f" ;;
    conformance-report) sed -n -e 's/^conformance-report: \([0-9]*\) tests, \([0-9]*\) ledger entries.*/suite=\1 ledger=\2 /p' \
                               -e 's/^conformance-report: [0-9]* normative statements, \([0-9]*\) gap.*/gaps=\1 /p' "$f" ;;
    l7-web) sed -n 's/^l7-web: \(L7c*d\) \([0-9]*\/[0-9]*\) .*/\1=\2 /p' "$f" ;;
    browser-pay-for-use) sed -n -e 's/^B1′ = .* = \([+-]*[0-9]*\) B.*/b1p=\1 /p' \
                          -e 's/^B13 = .* = \([+-]*[0-9]*\) B.*/b13=\1 /p' "$f" ;;
    native-no-heavy-crates) sed -n 's/^native-no-heavy-crates: \([0-9]*\) feature sets linked.*/sets=\1 /p' "$f" ;;
    tui-allocs-vs-pseudotrippy) awk -F'|' '$2 ~ /`tui-mf2`/ { s = $3; a = $4; u = $6 } $2 ~ /`tui-mf2 \(baseline\)`/ { b = $6 }
                END { gsub(/ /, "", s); gsub(/ /, "", a); gsub(/ /, "", u); gsub(/ /, "", b)
                      if (s != "") printf "tui=%s allocs=%s us=%s ", s, a, u; if (b != "") printf "base_us=%s", b }' "$f" ;;
  esac | tr -d '\n'
}

# pass or FAIL: the exit status, and for measure.sh and e2e.sh, which exit 0
# whatever their steps did, the steps' statuses and figures in the log.
judge() {
  local c=$1 f=$out/$1.log
  if [ "$2" != 0 ]; then echo FAIL; return; fi
  case $c in
    sizes) if grep -q ' rc=[1-9]' "$f" || ! grep -q '| B7 brotli' "$f" ||
              [ "$(figures sizes | grep -oE '(b1|b5|app|b5v)=[0-9]' | wc -l)" != 4 ]; then echo FAIL; return; fi ;;
    e2e-*) if grep -q ' rc=[1-9]' "$f" || [ "$(grep -cE ' check [a-z0-9]+ rc=0:' "$f")" != 6 ]; then echo FAIL; return; fi ;;
    demos) if [ ! -s "$out/demo-files.sha256" ]; then echo FAIL; return; fi ;;
  esac
  echo pass
}

minutes() { awk -v s="$1" 'BEGIN { printf "%.1f", s / 60 }'; }

record() {  # CHECK RC SECONDS, replacing the check's earlier row
  { if [ -f "$out/status.tsv" ]; then awk -F'\t' -v c="$1" '$1 != c' "$out/status.tsv"; fi
    printf '%s\t%s\t%s\n' "$@"; } >"$out/status.new"
  mv "$out/status.new" "$out/status.tsv"
}

summarize() {
  local c rc secs r failed=0
  [ -f "$out/status.tsv" ] || { echo "nothing has run under $label" >&2; return 2; }
  printf 'check\tresult\tminutes\tfigures\n' >"$out/summary.tsv"
  for c in "${all[@]}"; do
    read -r rc secs < <(awk -F'\t' -v c="$c" '$1 == c { print $2, $3 }' "$out/status.tsv") || continue
    r=$(judge "$c" "$rc"); [ "$r" = pass ] || failed=1
    printf '%s\t%s\t%s\t%s\n' "$c" "$r" "$(minutes "$secs")" "$(figures "$c" | sed 's/ *$//')" >>"$out/summary.tsv"
  done
  echo
  awk -F'\t' '{ printf "%-18s %-6s %7s  %s\n", $1, $2, $3, $4 }' "$out/summary.tsv"
  return $failed
}

if [ -z "$summarize" ]; then
  { git log --oneline -1; git status --short; } >"$out/commit.txt"
  echo "$label at $(git log --oneline -1 | cut -c1-60); logs in $out/"
  for c in "${all[@]}"; do
    [ -z "$only" ] || [[ ",$only," == *",$c,"* ]] || continue
    start=$(date +%s)
    exec_check "$c" >"$out/$c.log" 2>&1
    rc=$?
    secs=$(( $(date +%s) - start ))
    record "$c" "$rc" "$secs"
    printf '%s  %-18s %-4s %5s min\n' "$(date +%H:%M)" "$c" "$(judge "$c" "$rc")" "$(minutes "$secs")"
  done
fi
summarize
