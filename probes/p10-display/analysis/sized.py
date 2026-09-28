#!/usr/bin/env python3
"""match.py's changed functions with their body sizes in bytes.
  sized.py M_BASE.wasm M_VARIANT.wasm M_BASE.wat M_VARIANT.wat N_BASE.wat N_VARIANT.wat"""
import os, re, sys
from collections import Counter, defaultdict
here = os.path.dirname(os.path.abspath(__file__))
ns = {}
exec(open(os.path.join(here, "bodies.py")).read().replace("\nmain()\n", "\n"), ns)
mo = {}
exec(open(os.path.join(here, "match.py")).read().replace("\nmain()\n", "\n"), mo)

def imported_funcs(wat):
    n = 0
    with open(wat, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith(" (import ") and "(func " in line:
                n += 1
    return n

def sized(wasm, wat):
    bodies = ns["bodies"](open(wasm, "rb").read())
    imp = imported_funcs(wat)
    fs = mo["functions"](wat)
    assert len(fs) == len(bodies), (len(fs), len(bodies))
    return [(name, norm, len(bodies[i])) for i, (name, _, norm) in enumerate(fs)], imp

mbw, mvw, mbt, mvt, nbt, nvt = sys.argv[1:7]
mb, ib = sized(mbw, mbt)
mv, iv = sized(mvw, mvt)
print(f"imported functions: {ib} -> {iv}")
names = defaultdict(set)
for label, path in (("named base", nbt), ("named variant", nvt)):
    for name, _, norm in mo["functions"](path):
        names[norm].add((label, mo["unescape"](name)))
cb = Counter(n for _, n, _ in mb); cv = Counter(n for _, n, _ in mv)
for kind, diff, fs in (("base only", cb - cv, mb), ("variant only", cv - cb, mv)):
    print(kind)
    for name, norm, size in fs:
        if diff.get(norm):
            found = sorted(names.get(norm, set()))
            label = "; ".join(f"{l}: {n[:150]}" for l, n in found[:2]) or "(no named twin)"
            more = f" (+{len(found) - 2} more of this shape)" if len(found) > 2 else ""
            print(f"  {size:>4} B  {label}{more}")
