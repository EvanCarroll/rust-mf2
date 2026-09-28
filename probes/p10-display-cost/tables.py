#!/usr/bin/env python3
"""A9: the record's compact tables, from target/a9/results.tsv (the last
figure of each lib/client/case; every file of a build together).

    tables.py split CLIENT          Δ gz over base, form × type
    tables.py apps                  the cost in each application
    tables.py variants [--libs L,…] CASE...   each variant's figure against a5's
"""
import pathlib
import subprocess
import sys

root = pathlib.Path(
    subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
)
T = {}
for line in open(root / "target/a9/results.tsv"):
    lib, client, case, file, raw, gz, br, code, data = line.rstrip("\n").split("\t")
    if file == "total":
        T[(lib, client, case)] = (int(raw), int(gz), int(br))

KINDS = ["tr", "trargs", "trrich", "trdyn"]
NAMES = {"tr": "`Tr`", "trargs": "`TrArgs`", "trrich": "`TrRich`", "trdyn": "`TrDyn`"}
CLIENTS = ["fixture", "tr", "tr-view", "demo-csr", "demo-ssr", "demo-islands"]


def d(lib, client, case, against="base", i=1):
    a, b = T.get((lib, client, case)), T.get((lib, client, against))
    return None if a is None or b is None else a[i] - b[i]


def cell(v):
    return "—" if v is None else f"{v:+,}"


def split(client):
    print(f"Δ gz against `base` ({client}); raw in brackets\n")
    print("| form | " + " | ".join(NAMES[k] for k in KINDS) + " |")
    print("|---|" + "---:|" * len(KINDS))
    for form, label in [("display", "`{}`"), ("debug", "`{:?}`"), ("both", "`{}` and `{:?}`"), ("tostring", "`.to_string()`")]:
        cells = []
        for k in KINDS:
            g, r = d("a5", client, f"{form}-{k}"), d("a5", client, f"{form}-{k}", i=0)
            cells.append("—" if g is None else f"{g:+,} ({r:+,})")
        print(f"| {label} | " + " | ".join(cells) + " |")
    cells = []
    for k in KINDS:
        g, r = d("a5", client, f"display-{k}", f"tostring-{k}"), d("a5", client, f"display-{k}", f"tostring-{k}", i=0)
        cells.append("—" if g is None else f"{g:+,} ({r:+,})")
    print("| `{}` over `.to_string()` | " + " | ".join(cells) + " |")


def apps():
    print("Δ gz of every shipped file, against the client's `base`\n")
    print("| client | `base` raw / gz | `{}` `Tr` | `{}` over `.to_string()`, `Tr` / `TrArgs` | `{:?}` `Tr` | `{:?}` `TrArgs` | A5's control |")
    print("|---|---:|---:|---:|---:|---:|---:|")
    for c in CLIENTS:
        b = T.get(("a5", c, "base"))
        if not b:
            continue
        over = f"{cell(d('a5', c, 'display-tr', 'tostring-tr'))} / {cell(d('a5', c, 'display-trargs', 'tostring-trargs'))}"
        print(f"| {c} | {b[0]:,} / {b[1]:,} | {cell(d('a5', c, 'display-tr'))} | {over} | "
              f"{cell(d('a5', c, 'debug-tr'))} | {cell(d('a5', c, 'debug-trargs'))} | {cell(d('a5', c, 'control'))} |")


def variants(cases, libs=None):
    libs = libs or sorted({lib for lib, _, _ in T if lib != "a5"})
    print("Each variant against a5, the same client and case: Δ raw / Δ gz (— not built)\n")
    print("| client | case | a5 raw / gz | " + " | ".join(libs) + " |")
    print("|---|---|---:|" + "---:|" * len(libs))
    for c in CLIENTS:
        for case in cases:
            a = T.get(("a5", c, case))
            if not a or not any((lib, c, case) in T for lib in libs):
                continue
            cells = []
            for lib in libs:
                v = T.get((lib, c, case))
                cells.append("—" if v is None else f"{v[0] - a[0]:+,} / {v[1] - a[1]:+,}")
            print(f"| {c} | {case} | {a[0]:,} / {a[1]:,} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    what = sys.argv[1]
    if what == "split":
        split(sys.argv[2])
    elif what == "apps":
        apps()
    elif what == "variants":
        # tables.py variants [--libs a,b] CASE...
        args = sys.argv[2:]
        libs = None
        if args[:1] == ["--libs"]:
            libs, args = args[1].split(","), args[2:]
        variants(args or ["base", "display-tr", "display-trargs", "debug-tr", "debug-trargs", "control"], libs)
