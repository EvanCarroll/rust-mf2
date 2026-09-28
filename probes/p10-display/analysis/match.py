#!/usr/bin/env python3
"""A5 analysis: which functions differ between two stripped wasm modules
(measured base and variant, disassembled by wasm-dis), named by matching each
one's normalized text against two builds that kept their names.

  match.py M_BASE.wat M_VARIANT.wat N_BASE.wat N_VARIANT.wat

Normalized: every `$name` or `$index` (functions, locals, labels, types,
globals) becomes `$_`, and every numeric constant `_`, so that a renumbered
function or a moved table slot or data address is not a difference; what is
left is each function's instruction structure.
"""
import re
import sys
from collections import Counter, defaultdict

NAME = re.compile(r"\$[^\s()]+")
CONST = re.compile(r"\((i32|i64|f32|f64)\.const [^)]*\)")
OFFSET = re.compile(r"\b(offset|align)=\d+")


def functions(path):
    """(name, raw text, normalized text) per function, in module order."""
    out = []
    name = None
    lines = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith(" (") and not line.startswith("  "):
                if name is not None:
                    out.append(finish(name, lines))
                    name = None
                if line.startswith(" (func "):
                    name = line.split()[1]
                    lines = [line]
                continue
            if name is not None:
                lines.append(line)
    if name is not None:
        out.append(finish(name, lines))
    return out


def finish(name, lines):
    raw = "".join(lines)
    norm = OFFSET.sub(r"\1=_", CONST.sub(r"(\1.const _)", NAME.sub("$_", raw)))
    return name, raw, norm


def unescape(name):
    return re.sub(r"\\([0-9a-f]{2})", lambda m: chr(int(m.group(1), 16)), name)


def main():
    mb, mv, nb, nv = (functions(p) for p in sys.argv[1:5])
    print(f"functions: measured {len(mb)} -> {len(mv)}; named {len(nb)} -> {len(nv)}")
    cb = Counter(n for _, _, n in mb)
    cv = Counter(n for _, _, n in mv)
    gone = cb - cv
    new = cv - cb
    names = defaultdict(set)
    for label, fs in (("named base", nb), ("named variant", nv)):
        for name, _, norm in fs:
            names[norm].add((label, unescape(name)))

    def show(kind, diff):
        print(f"{kind}: {sum(diff.values())} function text(s)")
        for norm, count in sorted(diff.items(), key=lambda kv: -len(kv[0])):
            lines = norm.count("\n")
            found = sorted(names.get(norm, set()))
            print(f"  x{count}, {lines} lines of text")
            if not found:
                print("    no named build has this text")
            for label, name in found[:6]:
                print(f"    {label}: {name[:230]}")
            if len(found) > 6:
                print(f"    ... {len(found) - 6} more")

    show("only in measured base", gone)
    show("only in measured variant", new)


main()
