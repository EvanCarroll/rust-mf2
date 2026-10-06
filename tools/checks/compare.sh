#!/usr/bin/env bash
# Phase 10: two runs of `run.sh`, before (A) and after (B), side by side —
# each check's result in both, B's figures with their change from A — then
# the flags. Exits 1 if anything is flagged.
#
#   bash tools/checks/compare.sh A B     # labels under target/p10-checks/
#
# Flags: a check that fails in B; B1 beyond ±57 B br; B5 beyond ±0.18 B a site;
# any other size that moved; demo files that changed (demo-ssr's __wasm_split
# loader aside: its hash varies between identical builds); stripped tui-mf2
# above 1,381,752 B; allocations per frame that changed. Frame time is shown,
# not judged: it is taken under load.
set -u
cd "$(dirname "$0")/../.."
a=${1:?usage: compare.sh A B}; b=${2:?usage: compare.sh A B}
da=target/p10-checks/$a db=target/p10-checks/$b
for d in "$da" "$db"; do [ -f "$d/summary.tsv" ] || { echo "no $d/summary.tsv" >&2; exit 2; }; done
demo=
if [ -f "$da/demo-files.sha256" ] && [ -f "$db/demo-files.sha256" ]; then
  demo=$(awk '$2 ~ /__wasm_split/ { next } FILENAME == ARGV[1] { h[$2] = $1; next }
              { if (h[$2] != $1) print $2; delete h[$2] } END { for (p in h) print p }' \
           "$da/demo-files.sha256" "$db/demo-files.sha256" | sort | paste -sd' ')
fi
printf '%-18s %-9s %-9s %s\n' check "$a" "$b" "figures in $b (change from $a)"
awk -F'\t' -v B="$b" -v demo="$demo" '
  function isnum(x) { return x ~ /^[+-]?[0-9]+(\.[0-9]+)?$/ }
  function abs(x) { return x < 0 ? -x : x }
  function show(d) { return (d > 0 ? "+" : "") (d == int(d) ? sprintf("%d", d) : sprintf("%.1f", d)) }
  function flag(s) { flags[++nf] = s }
  FNR == 1 { next }
  FILENAME == ARGV[1] { ra[$1] = $2; fa[$1] = $4; next }
  {
    c = $1; inb[c] = 1
    if ($2 != "pass") flag(c " fails in " B)
    split("", old); n = split(fa[c], kv, " ")
    for (i = 1; i <= n; i++) { p = index(kv[i], "="); old[substr(kv[i], 1, p - 1)] = substr(kv[i], p + 1) }
    line = ""; n = split($4, kv, " ")
    for (i = 1; i <= n; i++) {
      p = index(kv[i], "="); k = substr(kv[i], 1, p - 1); v = substr(kv[i], p + 1); s = kv[i]
      if ((k in old) && old[k] != v) {
        w = old[k]
        if (isnum(v) && isnum(w)) {
          d = v - w; s = s " (" show(d) ")"
          if (k == "b1" && abs(d) > 57) flag(c ": B1 " w " -> " v " B br, beyond +-57")
          if (k == "b5" && abs(d) > 0.18 + 1e-9) flag(c ": B5 " w " -> " v " B br a site, beyond +-0.18")
          if (k ~ /^(app|b5v|b7\..*|rlib|b1p|b13)$/) flag(c ": " k " moved " w " -> " v " (" show(d) ")")
        } else {
          s = s " (was " w ")"
          if (k == "allocs") flag(c ": allocations per frame " w " -> " v)
        }
      }
      if (k == "tui" && v + 0 > 1381752) flag(c ": stripped tui-mf2 " v " B, above 1381752")
      line = line " " s
    }
    if (c == "demos" && demo != "") { line = line " changed: " demo; flag("demos: changed " demo) }
    printf "%-18s %-9s %-9s%s\n", c, ((c in ra) ? ra[c] : "-"), $2, line
  }
  END {
    for (c in ra) if (!(c in inb)) printf "%-18s %-9s %-9s\n", c, ra[c], "-"
    print ""
    if (!nf) { print "no flags"; exit 0 }
    for (i = 1; i <= nf; i++) print "! " flags[i]
    exit 1
  }' "$da/summary.tsv" "$db/summary.tsv"
