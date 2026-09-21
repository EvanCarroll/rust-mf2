#!/usr/bin/env python3
"""P0.1 analysis aid: raw and gzip -9 size of function bodies grouped by kind,
from a wasm file that still has its name section (scripts/names-build.sh)."""
import gzip, re, sys, collections
def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 0x7f) << s; s += 7
        if x < 0x80: return r, i
def parse(path):
    b = open(path, 'rb').read(); i = 8; secs = []
    while i < len(b):
        sid = b[i]; i += 1; n, i = leb(b, i); secs.append((sid, b[i:i+n])); i += n
    nimp = 0; bodies = []; names = {}
    for sid, s in secs:
        if sid == 2:
            j = 0; cnt, j = leb(s, j)
            for _ in range(cnt):
                l, j = leb(s, j); j += l; l, j = leb(s, j); j += l
                kind = s[j]; j += 1
                if kind == 0: _, j = leb(s, j); nimp += 1
                elif kind == 1: j += 1; fl, j = leb(s, j); _, j = leb(s, j); j = (leb(s, j)[1] if fl & 1 else j)
                elif kind == 2: fl, j = leb(s, j); _, j = leb(s, j); j = (leb(s, j)[1] if fl & 1 else j)
                elif kind == 3: j += 2
                elif kind == 4: j += 1; _, j = leb(s, j)
        elif sid == 10:
            j = 0; cnt, j = leb(s, j)
            for _ in range(cnt):
                l, j = leb(s, j); bodies.append(s[j:j+l]); j += l
        elif sid == 0:
            j = 0; l, j = leb(s, j); nm = s[j:j+l]; j += l
            if nm != b'name': continue
            while j < len(s):
                sub = s[j]; j += 1; l, j = leb(s, j); end = j + l
                if sub == 1:
                    k = j; cnt, k = leb(s, k)
                    for _ in range(cnt):
                        idx, k = leb(s, k); ln, k = leb(s, k); names[idx] = s[k:k+ln].decode('utf8', 'replace'); k += ln
                j = end
    return [(names.get(nimp + i, ''), body) for i, body in enumerate(bodies)]
def kind(n):
    n = re.sub(r'\[[0-9a-f]+\]', '', n)
    if 'drop_glue' in n or 'drop_in_place' in n: return 'drop_glue'
    for key in ['hydrate_async', 'hydrate_from_server', 'hydrate', 'rebuild', 'build', 'into_owned', 'resolve', 'mount', 'insert_before_this', 'elements']:
        if key in n: return key
    if 'mf2_probe' in n: return 'mf2_probe'
    if '::components::' in n: return 'components'
    return 'other'
res = {}
for p in sys.argv[1:]:
    g = collections.defaultdict(list)
    for n, body in parse(p): g[kind(n)].append(body)
    res[p] = {k: (sum(map(len, v)), len(gzip.compress(b''.join(v), 9))) for k, v in g.items()}
ks = sorted(set().union(*[r.keys() for r in res.values()]), key=lambda k: -res[sys.argv[1]].get(k, (0, 0))[0])
print('kind'.ljust(22) + ''.join(p.split('/')[-2][-26:].rjust(28) for p in res) + '   delta raw/gz')
for k in ks:
    vals = [res[p].get(k, (0, 0)) for p in res]
    print(k.ljust(22) + ''.join(f'{v[0]}/{v[1]}'.rjust(28) for v in vals) + f'   {vals[-1][0]-vals[0][0]:+d}/{vals[-1][1]-vals[0][1]:+d}')
