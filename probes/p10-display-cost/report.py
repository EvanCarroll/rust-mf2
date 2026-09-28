#!/usr/bin/env python3
"""A9: the tables of the record, from target/a9/results.tsv.

    report.py [CLIENT ...]        (from the root of the tree)

For each client and library variant: every case against that variant's first
`base` (Δ raw / gz / br of every shipped file together), then `{}` against
the inherent `to_string()` per description type, which is what `Display`
itself adds once the description is built and its text path linked. A case
measured more than once shows its last figure; a `base` measured twice must
agree (the reproducibility check), and the script says whether it does.
"""
import collections
import pathlib
import subprocess
import sys

root = pathlib.Path(
    subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
)
rows = [l.rstrip("\n").split("\t") for l in open(root / "target/a9/results.tsv") if l.strip()]
totals = collections.OrderedDict()
bases = collections.defaultdict(list)
for lib, client, case, file, raw, gz, br, code, data in rows:
    if file != "total":
        continue
    v = tuple(int(x) for x in (raw, gz, br, code, data))
    totals[(lib, client, case)] = v
    if case == "base":
        bases[(lib, client)].append(v)

want = sys.argv[1:]
for (lib, client), bs in bases.items():
    if want and client not in want:
        continue
    b = bs[0]
    same = all(x == b for x in bs)
    print(f"\n## {client} — library {lib} (base measured {len(bs)}×: {'identical' if same else 'DIFFERENT ' + str(bs)})")
    print("| case | raw | gz | br | Δ raw | Δ gz | Δ br | Δ code | Δ data |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for (l, c, case), v in totals.items():
        if (l, c) != (lib, client):
            continue
        d = [v[i] - b[i] for i in range(5)]
        print(f"| {case} | {v[0]:,} | {v[1]:,} | {v[2]:,} | {d[0]:+,} | {d[1]:+,} | {d[2]:+,} | {d[3]:+,} | {d[4]:+,} |")
    pairs = []
    for kind in ("tr", "trargs", "trrich", "trdyn"):
        dsp, tos = totals.get((lib, client, f"display-{kind}")), totals.get((lib, client, f"tostring-{kind}"))
        if dsp and tos:
            pairs.append((kind, [dsp[i] - tos[i] for i in range(5)]))
    if pairs:
        print("\n`{}` against the inherent `to_string()`, same description:\n")
        print("| type | Δ raw | Δ gz | Δ br | Δ code | Δ data |")
        print("|---|---:|---:|---:|---:|---:|")
        for kind, d in pairs:
            print(f"| {kind} | {d[0]:+,} | {d[1]:+,} | {d[2]:+,} | {d[3]:+,} | {d[4]:+,} |")
