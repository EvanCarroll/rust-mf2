#!/usr/bin/env python3
"""A9: sizes of shipped files — raw, gzip -9 and brotli q11 (the CLIs, as A5's
compressor table used them), and for a .wasm its code and data sections.

    measure.py LIB CLIENT CASE DIR    (every .wasm / .js under DIR)

Prints one TSV line per file and a `total` line, and appends them to
target/a9/results.tsv:
    lib client case file raw gz br code data
"""
import pathlib
import subprocess
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


def section_sizes(data):
    code = dat = 0
    i = 8
    while i < len(data):
        sid = data[i]
        size, j = leb(data, i + 1)
        if sid == 10:
            code = size
        elif sid == 11:
            dat = size
        i = j + size
    return code, dat


def compressed(path, cmd):
    out = subprocess.run(cmd + [str(path)], capture_output=True, check=True).stdout
    return len(out)


def main():
    lib, client, case, top = sys.argv[1:5]
    top = pathlib.Path(top)
    root = pathlib.Path(
        subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    )
    files = sorted(p for p in top.rglob("*") if p.is_file() and p.suffix in (".wasm", ".js"))
    rows = []
    total = [0, 0, 0, 0, 0]
    for p in files:
        data = p.read_bytes()
        raw = len(data)
        gz = compressed(p, ["gzip", "-9", "-n", "-c"])
        br = compressed(p, ["brotli", "-q", "11", "-c"])
        code, dat = section_sizes(data) if p.suffix == ".wasm" else (0, 0)
        rel = str(p.relative_to(top))
        rows.append([lib, client, case, rel, raw, gz, br, code, dat])
        for k, v in enumerate([raw, gz, br, code, dat]):
            total[k] += v
    rows.append([lib, client, case, "total", *total])
    with open(root / "target/a9/results.tsv", "a") as f:
        for r in rows:
            line = "\t".join(str(x) for x in r)
            print(line)
            f.write(line + "\n")


if __name__ == "__main__":
    main()
