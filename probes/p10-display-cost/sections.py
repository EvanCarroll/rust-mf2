#!/usr/bin/env python3
"""A9: a wasm module's sections and their sizes (custom sections by name).

    sections.py FILE [FILE ...]
"""
import sys

NAMES = {0: "custom", 1: "type", 2: "import", 3: "function", 4: "table", 5: "memory",
         6: "global", 7: "export", 8: "start", 9: "element", 10: "code", 11: "data",
         12: "datacount", 13: "tag"}


def leb(b, i):
    r = s = 0
    while True:
        x = b[i]
        i += 1
        r |= (x & 0x7F) << s
        s += 7
        if x < 0x80:
            return r, i


def sections(path):
    b = open(path, "rb").read()
    i = 8
    out = []
    while i < len(b):
        sid = b[i]
        size, j = leb(b, i + 1)
        name = NAMES.get(sid, str(sid))
        if sid == 0:
            n, k = leb(b, j)
            name = "custom:" + b[k:k + n].decode("utf-8", "replace")
        out.append((name, size))
        i = j + size
    return len(b), out


for path in sys.argv[1:]:
    total, secs = sections(path)
    print(f"{path}: {total} B")
    for name, size in secs:
        print(f"  {name:<28} {size:>9}")
