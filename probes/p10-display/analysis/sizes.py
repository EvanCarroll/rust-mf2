#!/usr/bin/env python3
"""A5 analysis: section sizes and the multiset difference of function-body
sizes between two wasm modules (no names needed).  sizes.py BASE VARIANT"""
import sys
from collections import Counter
sys.path.insert(0, __import__("os").path.dirname(__file__))
from importlib.machinery import SourceFileLoader
src = open(__import__("os").path.join(__import__("os").path.dirname(__file__), "bodies.py")).read().replace("\nmain()\n", "\n")
ns = {}
exec(compile(src, "bodies.py", "exec"), ns)
a = open(sys.argv[1], "rb").read(); b = open(sys.argv[2], "rb").read()
sa = {n: s for n, _, _, s in ns["sections"](a)}; sb = {n: s for n, _, _, s in ns["sections"](b)}
for n in dict.fromkeys(list(sa) + list(sb)):
    if sa.get(n, 0) != sb.get(n, 0):
        print(f"  section {n}: {sa.get(n,0)} -> {sb.get(n,0)} ({sb.get(n,0)-sa.get(n,0):+d})")
ba, bb = ns["bodies"](a), ns["bodies"](b)
print(f"  functions: {len(ba)} -> {len(bb)}")
ca, cb = Counter(len(x) for x in ba), Counter(len(x) for x in bb)
gone, new = ca - cb, cb - ca
print(f"  body sizes only in base:    {sorted(gone.elements(), reverse=True)} (sum {sum(gone.elements())})")
print(f"  body sizes only in variant: {sorted(new.elements(), reverse=True)} (sum {sum(new.elements())})")
