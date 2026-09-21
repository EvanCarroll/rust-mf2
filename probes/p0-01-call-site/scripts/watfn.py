#!/usr/bin/env python3
"""Print wat functions whose (unescaped) name contains all given substrings."""
import re, sys
path, pats = sys.argv[1], sys.argv[2:]
limit = 3
def unesc(s):
    return re.sub(r'\\([0-9a-f]{2})', lambda m: chr(int(m.group(1), 16)), s)
out = []; cur = None; n = 0
for line in open(path):
    if line.startswith(' (func '):
        if cur is not None:
            print(''.join(cur)); n += 1
            if n >= limit: sys.exit(0)
        name = re.sub(r'\[[0-9a-f]+\]', '', unesc(line.split(' ')[2]))
        cur = [line[:300] + '\n'] if all(p in name for p in pats) else None
    elif cur is not None:
        cur.append(line)
if cur: print(''.join(cur))
