#!/usr/bin/env python3
"""A5 analysis: compare two wasm modules function by function, without names.

Usage: bodies.py BASE.wasm VARIANT.wasm

Prints each section's size, the function count, and the multiset difference
of the code section's function bodies: a body is keyed by its bytes with every
`call N` target replaced by a placeholder (so that a shifted function index
does not count as a change), and reported with its size.
"""
import hashlib
import sys
from collections import Counter


def leb(buf, i):
    result = shift = 0
    while True:
        b = buf[i]
        i += 1
        result |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return result, i


def sections(buf):
    assert buf[:4] == b"\0asm"
    i = 8
    out = []
    while i < len(buf):
        sid = buf[i]
        size, j = leb(buf, i + 1)
        name = str(sid)
        if sid == 0:
            nlen, k = leb(buf, j)
            name = "custom:" + buf[k:k + nlen].decode()
        out.append((name, sid, j, size))
        i = j + size
    return out


def bodies(buf):
    for name, sid, start, size in sections(buf):
        if sid != 10:
            continue
        count, i = leb(buf, start)
        result = []
        for _ in range(count):
            bsize, j = leb(buf, i)
            result.append(bytes(buf[j:j + bsize]))
            i = j + bsize
        return result
    return []


def key(body):
    # Replace the LEB operand of every 0x10 (call) byte that is followed by a
    # valid LEB: coarse (a 0x10 byte inside an immediate is also rewritten),
    # but applied alike to both modules.
    out = bytearray()
    i = 0
    while i < len(body):
        b = body[i]
        out.append(b)
        i += 1
        if b == 0x10 and i < len(body):
            j = i
            while j < len(body) and body[j] & 0x80:
                j += 1
            i = j + 1
            out.append(0xFF)
    return hashlib.sha1(bytes(out)).hexdigest()[:12]


def main():
    a = open(sys.argv[1], "rb").read()
    b = open(sys.argv[2], "rb").read()
    sa = {n: s for n, _, _, s in sections(a)}
    sb = {n: s for n, _, _, s in sections(b)}
    print("section sizes (base -> variant):")
    for n in list(dict.fromkeys(list(sa) + list(sb))):
        x, y = sa.get(n, 0), sb.get(n, 0)
        mark = "" if x == y else f"   ({y - x:+d})"
        print(f"  {n:>28}: {x:>9} -> {y:>9}{mark}")
    ba, bb = bodies(a), bodies(b)
    print(f"functions: {len(ba)} -> {len(bb)}")
    ca = Counter((key(x), len(x)) for x in ba)
    cb = Counter((key(x), len(x)) for x in bb)
    gone = ca - cb
    new = cb - ca
    print(f"bodies only in base: {sum(gone.values())}, "
          f"{sum(k[1] * n for k, n in gone.items())} B")
    for (h, n), c in sorted(gone.items(), key=lambda kv: -kv[0][1]):
        print(f"  - {n:>6} B  x{c}  {h}")
    print(f"bodies only in variant: {sum(new.values())}, "
          f"{sum(k[1] * n for k, n in new.items())} B")
    for (h, n), c in sorted(new.items(), key=lambda kv: -kv[0][1]):
        print(f"  + {n:>6} B  x{c}  {h}")


main()
