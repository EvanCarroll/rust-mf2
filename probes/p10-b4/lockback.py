#!/usr/bin/env python3
"""Phase 10 B4: whether an application's lock lost `leptos-mf2` and nothing else.

    python3 probes/p10-b4/lockback.py WORKLOAD APP HASHES

WORKLOAD/APP/Cargo.lock is the lock after the build with B4's template;
HASHES is the list `regen.sh` wrote before generating WORKLOAD again
(`target/p10-b2/logs/LABEL/<workload>.before.sha256`), which holds the lock's
hash from before. `leptos-mf2`'s entry (1.1.0, depending on `mf2`) goes back
where cargo sorts it, and the line listing it back into APP's dependencies;
the result's hash must be the one from before.
"""
import hashlib
import pathlib
import re
import sys


def package_name(block):
    return re.search(r'^name = "([^"]+)"', block, re.M).group(1)


def main():
    workload, app, hashes = sys.argv[1:4]
    lock = pathlib.Path(workload, app, "Cargo.lock").read_text()
    manifest = pathlib.Path(workload, app, "Cargo.toml").read_text()
    krate = re.search(r'^name = "([^"]+)"', manifest, re.M).group(1)
    want = next(
        line.split()[0]
        for line in pathlib.Path(hashes).read_text().splitlines()
        if line.endswith(f"./{app}/Cargo.lock")
    )
    if 'name = "leptos-mf2"' in lock:
        raise SystemExit("the lock still names leptos-mf2")
    head, *blocks = lock.split("\n[[package]]\n")
    at = next(i for i, b in enumerate(blocks) if package_name(b) > "leptos-mf2")
    blocks.insert(at, 'name = "leptos-mf2"\nversion = "1.1.0"\ndependencies = [\n "mf2",\n]\n')
    for n, block in enumerate(blocks):
        if package_name(block) != krate:
            continue
        lines = block.split("\n")
        start = lines.index("dependencies = [")
        end = lines.index("]", start)
        deps = lines[start + 1:end]
        at = next(i for i, d in enumerate(deps) if d.strip(' ",').split(" ")[0] > "leptos-mf2")
        deps.insert(at, ' "leptos-mf2",')
        blocks[n] = "\n".join(lines[:start + 1] + deps + lines[end:])
    rebuilt = head + "".join("\n[[package]]\n" + b for b in blocks)
    same = hashlib.sha256(rebuilt.encode()).hexdigest() == want
    print(f"{workload}/{app}: the lock from before is this one with leptos-mf2 put back: {same}")
    sys.exit(0 if same else 1)


if __name__ == "__main__":
    main()
