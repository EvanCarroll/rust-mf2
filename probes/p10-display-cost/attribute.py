#!/usr/bin/env python3
"""A9: what a case adds to a client, by kind of code — twiggy over two
names-kept builds (run.sh with A9_NAMED=1), joined on normalised names.

    attribute.py BASE.wasm CASE.wasm [--list N]

Names are normalised so that renames cancel: crate hashes (`[0123abcd]`,
`17h0123…E`), binaryen's `.N` suffixes and closure numbers dropped. Each item
is put in the first group whose pattern matches its name; the groups are the
parts of `core::fmt` the dominators point to, and ours.
"""
import collections
import json
import re
import subprocess
import sys

INT = r"(u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)"
GROUPS = [
    ("ours: Display / Debug impls", r"<[^ ]*(leptos_mf2|mf2_runtime|mf2_model|mf2_catalog|mf2_[a-z_]+)(\[[0-9a-f]+\])?::[^ ]* as core(\[[0-9a-f]+\])?::fmt::(Display|Debug)>::fmt|leptos_mf2(\[[0-9a-f]+\])?::(display|debug|text::fmt_display)"),
    ("ours: other", r"leptos_mf2|mf2_runtime|mf2_model|mf2_catalog|mf2_fn_|mf2_host|\bmf2::"),
    ("the call site (the client's entry point)", r"::main$|::hydrate$|mf2_i18n_client|workload_app_|demo_ssr|demo_csr|demo_islands"),
    ("core::fmt float (flt2dec, Grisu, Dragon, 128-bit helpers)", r"flt2dec|grisu|dragon|fmt::float|float_to_(decimal|exponential|general)|<&?f(32|64) as core|bignum|dec2flt|__multi3|__udivti3|__umodti3"),
    ("core::fmt integers (num, pad_integral, hex)", r"fmt::num|pad_integral|LowerHex|UpperHex|DEC_DIGITS_LUT|<&?(core(\[[0-9a-f]+\])?::option::Option<)?&?" + INT + r">? as core"),
    ("str / char escaping, Unicode tables", r"escape|unicode|printable|grapheme|is_printable|<&?char as core|<&?str as core(\[[0-9a-f]+\])?::fmt::Debug|EscapeDebug|str::lossy"),
    ("Debug builders (DebugStruct, DebugTuple, DebugList, PadAdapter)", r"builders|DebugStruct|DebugTuple|DebugList|DebugInner|DebugSet|DebugMap|PadAdapter|debug_(struct|tuple|list)"),
    ("panic paths (and their messages' Debug)", r"panic|slice_error_fail|slice_index_fail|slice_end_index|slice_start_index|str_index_overflow|Range<usize> as core|unwrap_failed|expect_failed"),
    ("Formatter core (write, pad, padding, Arguments)", r"core(\[[0-9a-f]+\])?::fmt::write|Formatter|fmt::Arguments|core(\[[0-9a-f]+\])?::fmt::Write|fmt::write|padding|PostPadding|core(\[[0-9a-f]+\])?::fmt::rt|fmt::Error"),
    ("alloc::fmt / String as fmt::Write", r"alloc(\[[0-9a-f]+\])?::fmt|format_inner|String as core|write_str|write_char"),
    ("data (tables, strings; segments renumber)", r"^data segment|^data\["),
    ("unnamed code (renumbered by binaryen)", r"^code\["),
]


def norm(name):
    name = re.sub(r"\[[0-9a-f]{16}\]", "", name)
    name = re.sub(r"::h[0-9a-f]{16}", "", name)
    name = re.sub(r"17h[0-9a-f]{16}E", "", name)
    name = re.sub(r"\.\d+$", "", name)
    name = re.sub(r"\{\{closure\}\}(#\d+)?", "{closure}", name)
    return name


def items(path):
    out = subprocess.run(["twiggy", "top", "-n", "1000000", "-f", "json", path], capture_output=True, check=True, text=True).stdout
    table = collections.Counter()
    for it in json.loads(out):
        table[norm(it["name"])] += it["shallow_size"]
    return table


def group(name):
    for label, pat in GROUPS:
        if re.search(pat, name):
            return label
    return "other"


def main():
    base, case = sys.argv[1], sys.argv[2]
    n = int(sys.argv[sys.argv.index("--list") + 1]) if "--list" in sys.argv else 0
    a, b = items(base), items(case)
    by_group = collections.Counter()
    changes = []
    for name in set(a) | set(b):
        d = b.get(name, 0) - a.get(name, 0)
        if d and not name.startswith("\"function names\"") and "custom section" not in name and name not in ("\"names\"",):
            by_group[group(name)] += d
            changes.append((d, name))
    total = sum(by_group.values())
    print(f"{case} against {base}: {total:+d} B (names excluded)")
    for label, _ in GROUPS + [("other", "")]:
        if by_group.get(label):
            print(f"  {by_group[label]:+7d}  {label}")
    if n:
        print("  largest changes:")
        for d, name in sorted(changes, key=lambda x: -abs(x[0]))[:n]:
            print(f"  {d:+7d}  [{group(name)[:12]}] {name[:150]}")


if __name__ == "__main__":
    main()
