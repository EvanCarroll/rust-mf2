#!/usr/bin/env python3
"""P0.1: turn target/results.tsv into the RESULT.md tables.

Columns of results.tsv (scripts/build-app.sh):
  workload  template  cargo_raw  bindgen_raw  bindgen_gz  opt_raw  opt_gz

Method (RESULT.md): every figure is a delta against a baseline app of the
same workload: `idlit` (each site its MsgId as a short literal; the B5
baseline used for the verdict), `dummy` (one literal everywhere) or
`literal` (each site its source text). With two scales S1 < S2 of the same generator knobs ratio,
marginal per-site = (delta@S2 - delta@S1) / (n@S2 - n@S1), and the fixed
part = delta@S1 - n@S1 * marginal.

Usage: analyze.py [results.tsv] [--small W1 W2]
"""
import collections
import json
import os
import sys

PROBE = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))
T = os.path.join(PROBE, "target")
COLS = ["bindgen_raw", "bindgen_gz", "opt_raw", "opt_gz"]
SHAPES = ["string", "child", "attr", "text_prop", "signal_prop", "string_prop", "deferred", "if_else"]
MIX = {  # plans/06 §2 weights (%): text_prop + signal_prop = the 8 % reactive-prop share
    "string": 45, "child": 20, "attr": 8, "text_prop": 4, "signal_prop": 4,
    "string_prop": 4, "deferred": 8, "if_else": 7,
}
ARGMODES = ("plain", "signal", "get")


def load(path):
    rows = {}
    for line in open(path):
        p = line.rstrip("\n").split("\t")
        if len(p) < 7:
            continue
        rows[(p[0], p[1])] = dict(zip(COLS, map(int, p[3:7])))
    return rows


def counts(wl):
    sites = json.load(open(os.path.join(T, wl, "sites.json")))
    n = collections.Counter()
    for s in sites:
        n[(s["shape"], "all")] += 1
        n[(s["shape"], "args" if s["mode"] in ARGMODES else "noargs")] += 1
        n[(s["shape"], "mode:" + s["mode"])] += 1
        n[("*", "all")] += 1
    return n


def delta(rows, wl, tpl, base="dummy"):
    a, b = rows.get((wl, tpl)), rows.get((wl, base))
    if not a or not b:
        return None
    return {c: a[c] - b[c] for c in COLS}


def fmt(x, nd=1):
    return "—" if x is None else f"{x:.{nd}f}"


def main():
    args = sys.argv[1:]
    path = args[0] if args and not args[0].startswith("--") else os.path.join(T, "results.tsv")
    w1, w2 = "wl-1860", "wl-3720"
    if "--small" in args:
        i = args.index("--small")
        w1, w2 = args[i + 1], args[i + 2]
    rows = load(path)
    n1, n2 = counts(w1), counts(w2)
    m1, m2 = n1[("*", "all")], n2[("*", "all")]

    print(f"## Whole app (delta / M), {w1} (M={m1}) and {w2} (M={m2})\n")
    print("| template | baseline | M | bindgen raw | bindgen gz | opt raw | opt gz |")
    print("|---|---|---|---|---|---|---|")
    for tpl in ("tr", "closure", "literal", "dummy"):
        for base in ("idlit", "dummy", "literal"):
            if tpl == base or (tpl, base) in (("dummy", "literal"), ("literal", "dummy")):
                continue
            for wl, m in ((w1, m1), (w2, m2)):
                d = delta(rows, wl, tpl, base)
                if d:
                    print(f"| {tpl} | {base} | {m} | " + " | ".join(fmt(d[c] / m) for c in COLS) + " |")
    print("\n## Absolute sizes (bytes)\n")
    print("| workload | template | bindgen raw | bindgen gz | opt raw | opt gz |")
    print("|---|---|---|---|---|---|")
    for (wl, tpl), r in sorted(rows.items()):
        if "--" in tpl or wl not in (w1, w2):
            continue
        print(f"| {wl} | {tpl} | " + " | ".join(str(r[c]) for c in COLS) + " |")

    print(f"\n## Marginal per site and fixed cost (M-delta {m1} → {m2})\n")
    print("| template | baseline | marginal bindgen raw | gz | marginal opt raw | **opt gz** | fixed opt raw | fixed opt gz |")
    print("|---|---|---|---|---|---|---|---|")
    for tpl in ("tr", "closure", "literal", "dummy"):
        for base in ("idlit", "dummy"):
            if tpl == base:
                continue
            d1, d2 = delta(rows, w1, tpl, base), delta(rows, w2, tpl, base)
            if not d1 or not d2:
                continue
            mg = {c: (d2[c] - d1[c]) / (m2 - m1) for c in COLS}
            fx = {c: d1[c] - m1 * mg[c] for c in COLS}
            print(f"| {tpl} | {base} | " + " | ".join(fmt(mg[c]) for c in COLS) + f" | {fx['opt_raw']:.0f} | {fx['opt_gz']:.0f} |")

    print("\n## Per shape vs idlit (marginal B/site via M-delta; fixed = what the variant adds at M1 beyond n × marginal)\n")
    print("| impl | shape | sites M1/M2 | opt raw/site | **opt gz/site** | bindgen raw/site | bindgen gz/site | fixed opt gz |")
    print("|---|---|---|---|---|---|---|---|")
    per = collections.defaultdict(dict)
    for impl in ("tr", "closure"):
        for s in SHAPES:
            short = s.replace("_", "")
            for part, suffix in (("all", ""), ("noargs", "--none")):
                if part == "noargs" and s == "deferred":
                    continue
                v = f"{impl}--{short}{suffix}"
                d1, d2 = delta(rows, w1, v, "idlit"), delta(rows, w2, v, "idlit")
                k1, k2 = n1[(s, part)], n2[(s, part)]
                if not d1 or not d2 or k2 == k1:
                    continue
                mg = {c: (d2[c] - d1[c]) / (k2 - k1) for c in COLS}
                fx = d1["opt_gz"] - k1 * mg["opt_gz"]
                per[impl][(s, part)] = (mg, d1, d2)
                label = s if part == "all" else s + " (no args)"
                print(f"| {impl} | {label} | {k1}/{k2} | {fmt(mg['opt_raw'])} | {fmt(mg['opt_gz'])} | "
                      f"{fmt(mg['bindgen_raw'])} | {fmt(mg['bindgen_gz'])} | {fx:.0f} |")
            # argument sites = variant(all) - variant(none), marginal over the arg-site count
            a1, a2 = per[impl].get((s, "all")), per[impl].get((s, "noargs"))
            if a1 and a2:
                k1, k2 = n1[(s, "args")], n2[(s, "args")]
                if k2 > k1:
                    da1 = {c: a1[1][c] - a2[1][c] for c in COLS}
                    da2 = {c: a1[2][c] - a2[2][c] for c in COLS}
                    mg = {c: (da2[c] - da1[c]) / (k2 - k1) for c in COLS}
                    fx = da1["opt_gz"] - k1 * mg["opt_gz"]
                    per[impl][(s, "args")] = (mg, None, None)
                    print(f"| {impl} | {s} (args only) | {k1}/{k2} | {fmt(mg['opt_raw'])} | {fmt(mg['opt_gz'])} | "
                          f"{fmt(mg['bindgen_raw'])} | {fmt(mg['bindgen_gz'])} | {fx:.0f} |")

    print("\n## Weighted average over plans/06 §2's mix (per-shape marginals)\n")
    print("| impl | opt raw/site | **opt gz/site** | bindgen raw/site | bindgen gz/site |")
    print("|---|---|---|---|---|")
    for impl in ("tr", "closure"):
        if all((s, "all") in per[impl] for s in SHAPES):
            w = {c: sum(MIX[s] * per[impl][(s, "all")][0][c] for s in SHAPES) / 100 for c in COLS}
            print(f"| {impl} | " + " | ".join(fmt(w[c]) for c in (COLS[2], COLS[3], COLS[0], COLS[1])) + " |")


if __name__ == "__main__":
    main()
