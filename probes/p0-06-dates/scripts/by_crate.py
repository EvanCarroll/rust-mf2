#!/usr/bin/env python3
"""Group `twiggy top --format csv` rows by the first crate named in each symbol.
Usage: by_crate.py top.csv [N]"""
import csv, re, sys, collections
rows = list(csv.DictReader(open(sys.argv[1])))
n = int(sys.argv[2]) if len(sys.argv) > 2 else 25
agg = collections.Counter()
total = 0
for r in rows:
    name, b = r['Name'], int(r['ShallowSize'])
    if name.startswith('"function names"') or name.startswith('"'):
        if 'function names' in name:
            continue
    total += b
    m = re.search(r'([a-z_][a-z_0-9]*)\[[0-9a-f]{16}\]', name)
    if m:
        k = m.group(1)
    elif name.startswith('data segment'):
        k = 'data segments'
    else:
        k = 'other: ' + name[:40]
    agg[k] += b
print(f'total (excluding name section): {total}')
for k, v in agg.most_common(n):
    print(f'{v:8d} {100*v/total:5.1f}%  {k}')
