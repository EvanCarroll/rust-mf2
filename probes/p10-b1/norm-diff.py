#!/usr/bin/env python3
"""Phase 10 A7: twiggy's item tables of two named builds, joined on a
normalized name (crate hashes dropped; the helper crate's `ui` module mapped
onto leptos-mf2's `components`), so that renames cancel and what is left is
what changed. Usage: norm-diff.py base.top.json v3.top.json [rows]"""
import collections, json, re, sys

def norm(name):
    name = re.sub(r"\[[0-9a-f]{16}\]", "", name)
    name = name.replace("mf2_leptos_ui_0_9::ui::", "leptos_mf2::components::")
    name = name.replace("mf2_leptos_ui_0_8::ui::", "leptos_mf2::components::")
    # Phase 10 B1: the layer moved into mf2 (mf2::leptos::…), the core
    # files to mf2's root; mapped back onto 1.x's leptos_mf2 paths.
    name = re.sub(r"\bmf2::leptos::", "leptos_mf2::", name)
    name = re.sub(r"\bmf2::(tr|arg|dynamic|markup|display|debug)::", r"leptos_mf2::\1::", name)
    name = re.sub(r"\{closure#\d+\}", "{closure}", name)
    name = re.sub(r"_export_[0-9a-f]{32}_", "_export_H_", name)
    return name

def load(path):
    items = collections.Counter()
    for it in json.load(open(path)):
        items[norm(it["name"])] += it["shallow_size"]
    return items

base, v3 = load(sys.argv[1]), load(sys.argv[2])
rows = int(sys.argv[3]) if len(sys.argv) > 3 else 40
def is_code(n):
    return not (n.startswith("custom section") or "subsection" in n or n.startswith("data") or n.startswith("elem[") or n.startswith("table[") or n.startswith("type[") or n.startswith("import ") or n.startswith("export ") or n.startswith("global["))
delta = {n: v3.get(n, 0) - base.get(n, 0) for n in set(base) | set(v3)}
changed = sorted(((d, n) for n, d in delta.items() if d), key=lambda x: -abs(x[0]))
tot = lambda f: sum(d for n, d in delta.items() if f(n))
print(f"items: base {len(base)}, v3 {len(v3)}; changed after normalizing: {len(changed)}")
print(f"net, all items: {tot(lambda n: True):+d}")
print(f"net, custom sections and name subsections: {tot(lambda n: n.startswith('custom section') or 'subsection' in n):+d}")
print(f"net, data segments: {tot(lambda n: n.startswith('data')):+d}")
print(f"net, elem/table/type/import/export/global: {tot(lambda n: n.startswith(('elem[', 'table[', 'type[', 'import ', 'export ', 'global['))):+d}")
print(f"net, code (functions): {tot(is_code):+d}")
print()
for d, n in changed[:rows]:
    print(f"{d:+7d}  base {base.get(n, 0):6d}  v3 {v3.get(n, 0):6d}  {n[:170]}")
