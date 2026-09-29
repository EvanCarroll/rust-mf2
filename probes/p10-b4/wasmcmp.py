#!/usr/bin/env python3
"""Phase 10 B4: two builds of one wasm module compared section by section.

    python3 probes/p10-b4/wasmcmp.py BEFORE.wasm AFTER.wasm

For every section: its size in each, and whether its bytes are the same.
Where they differ, what differs: for imports and exports, the names; for
code, the function bodies (how many differ in place, and whether the two are
the same bodies in another order); for data of one length, how many bytes
differ.
"""
import collections
import hashlib
import sys


def leb(b, i):
    r = s = 0
    while True:
        x = b[i]
        i += 1
        r |= (x & 0x7F) << s
        s += 7
        if x < 0x80:
            return r, i


def name(b, i):
    n, i = leb(b, i)
    return b[i:i + n].decode(), i + n


def sections(b):
    if b[:4] != b"\0asm":
        raise SystemExit("not a wasm module")
    i, out = 8, []
    while i < len(b):
        sid = b[i]
        n, i = leb(b, i + 1)
        body = b[i:i + n]
        custom = name(body, 0)[0] if sid == 0 else None
        out.append((sid, custom, body))
        i += n
    return out


def vector(body, item):
    n, i = leb(body, 0)
    out = []
    for _ in range(n):
        x, i = item(body, i)
        out.append(x)
    return out


def body_of(b, i):
    size, i = leb(b, i)
    return b[i:i + size], i + size


def import_of(b, i):
    module, i = name(b, i)
    field, i = name(b, i)
    kind = b[i]
    if kind != 0:
        raise SystemExit(f"import kind {kind} not read")
    index, i = leb(b, i + 1)
    return (module, field, index), i


def export_names(body):
    n, i = leb(body, 0)
    out = []
    for _ in range(n):
        field, i = name(body, i)
        kind = body[i]
        index, i = leb(body, i + 1)
        out.append((field, kind, index))
    return out


def main():
    a, b = (open(p, "rb").read() for p in sys.argv[1:3])
    sa, sb = sections(a), sections(b)
    print(f"sizes: {len(a)} / {len(b)} bytes")
    if [(s, c) for s, c, _ in sa] != [(s, c) for s, c, _ in sb]:
        print("the sections differ in kind or order")
        return
    for (sid, custom, x), (_, _, y) in zip(sa, sb):
        label = f"{sid}" + (f" ({custom})" if custom else "")
        print(f"  section {label}: {len(x)} / {len(y)} bytes, {'same' if x == y else 'differs'}")
        if x == y:
            continue
        if sid == 2:
            for k, (p, q) in enumerate(zip(vector(x, import_of), vector(y, import_of))):
                if p != q:
                    print(f"    import {k}: {p[0]}::{p[1]} / {q[1]}")
        elif sid == 7:
            for k, (p, q) in enumerate(zip(export_names(x), export_names(y))):
                if p != q:
                    print(f"    export {k}: {p[0]} / {q[0]}")
        elif sid == 10:
            p, q = vector(x, body_of), vector(y, body_of)
            moved = [k for k, (u, v) in enumerate(zip(p, q)) if u != v]
            same_set = collections.Counter(map(lambda z: hashlib.sha256(z).digest(), p)) == \
                collections.Counter(map(lambda z: hashlib.sha256(z).digest(), q))
            print(f"    {len(p)} / {len(q)} functions; {len(moved)} differ in place; "
                  f"the same bodies in another order: {same_set and bool(moved)}")
            for k in moved[:10]:
                bytes_ = sum(1 for u, v in zip(p[k], q[k]) if u != v)
                print(f"      function {k}: {len(p[k])} / {len(q[k])} bytes, {bytes_} differ")
        elif sid == 11 and len(x) == len(y):
            differing = sum(1 for u, v in zip(x, y) if u != v)
            print(f"    {differing} bytes differ")


if __name__ == "__main__":
    main()
