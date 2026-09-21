#!/usr/bin/env python3
"""P0.1 analysis aid: raw and gzip -9 size of each wasm section, for several files."""
import gzip, sys
NAMES = {0:'custom',1:'type',2:'import',3:'function',4:'table',5:'memory',6:'global',7:'export',8:'start',9:'element',10:'code',11:'data',12:'datacount'}
def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 0x7f) << s; s += 7
        if x < 0x80: return r, i
def sections(path):
    b = open(path, 'rb').read(); i = 8; out = {}
    while i < len(b):
        sid = b[i]; i += 1; n, i = leb(b, i)
        k = NAMES.get(sid, str(sid)); out[k] = out.get(k, b'') + b[i:i+n]; i += n
    return out, b
rows = {}
for p in sys.argv[1:]:
    s, whole = sections(p)
    rows[p] = {k: (len(v), len(gzip.compress(v, 9))) for k, v in s.items()}
    rows[p]['WHOLE'] = (len(whole), len(gzip.compress(whole, 9)))
keys = [k for k in ['code', 'data', 'element', 'function', 'type', 'import', 'export', 'global', 'custom', 'WHOLE'] if any(k in r for r in rows.values())]
print('file'.ljust(40) + ''.join(k.rjust(22) for k in keys))
for p, r in rows.items():
    print(p[-40:].ljust(40) + ''.join((f"{r[k][0]}/{r[k][1]}" if k in r else '-').rjust(22) for k in keys))
