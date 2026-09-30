#!/usr/bin/env python3
"""Phase 10 A7: copies of the three demos on the Leptos 0.8 line, with a copy
of the e2e harness beside them, so that the checks which serve
`examples/demo-csr/dist/` relative to themselves serve the 0.8 copy's.

    target/a7-demo-0-8/<tag>/examples/{demo-ssr,demo-islands,demo-csr}
    target/a7-demo-0-8/<tag>/tools/e2e   (checks, lib, run.mjs; node_modules linked)

The tree builds the demos on Leptos 0.9 only; its 0.8 check is `cargo xtask
leptos-0-8`. The conversion:
* path dependencies made absolute (the copies live under `target/`);
* the Leptos crates at their 0.8 releases, and `mf2`'s `leptos` feature
  replaced by `leptos-0-8`;
* demo-ssr: `leptos/lazy` dropped from `hydrate` — Leptos 0.8.21 has no such
  feature (`cargo info leptos@0.8.21`), and a copy that keeps it fails in
  `cargo metadata` at `2fb7f54` and on the branch alike (`--keep-lazy`
  reproduces that);
* demo-csr: Trunk.toml's `mf2-cli` hook pointed at this tree's manifest.
Each demo's `Cargo.lock` is copied with it (cargo then resolves the 0.8
crates).

Usage, from anywhere: demos-0-8.py <tag> [--keep-lazy]
The copies are of whatever the tree holds when it runs. Then, in each copy,
`cargo leptos build [--split]` / `trunk build`, a server on the demo's own
port, and `node run.mjs <check>` in `target/a7-demo-0-8/<tag>/tools/e2e`."""
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
KEEP_LAZY = "--keep-lazy" in sys.argv[1:]
TAG = ARGS[0] if ARGS else "head"
OUT = ROOT / "target/a7-demo-0-8" / TAG
CRATES = str(ROOT / "crates")


def must_replace(text, old, new, what):
    if old not in text:
        sys.exit(f"{what}: {old!r} not found")
    return text.replace(old, new)


for demo in ["demo-ssr", "demo-islands", "demo-csr"]:
    src = ROOT / "examples" / demo
    dst = OUT / "examples" / demo
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst, ignore=shutil.ignore_patterns("target", "dist"))
    for manifest in [dst / "Cargo.toml", dst / "i18n/Cargo.toml"]:
        text = manifest.read_text()
        text = text.replace('path = "../../../crates', f'path = "{CRATES}')
        text = text.replace('path = "../../crates', f'path = "{CRATES}')
        if manifest.parent == dst:
            what = f"{demo}/Cargo.toml"
            text = text.replace('version = "0.9.0-beta"', 'version = "0.8"')
            text = text.replace('leptos_meta = "0.9.0-beta"', 'leptos_meta = "0.8"')
            text = text.replace('leptos_router = "0.9.0-beta"', 'leptos_router = "0.8"')
            # `mf2`'s Leptos line: `leptos-0-8` in place of `leptos` (Phase 10 D6).
            text = must_replace(text, 'features = ["leptos", ', 'features = ["leptos-0-8", ', what)
            if demo == "demo-ssr" and not KEEP_LAZY:
                text = must_replace(
                    text,
                    '    # Leptos 0.9 hydrates the `#[lazy_route]` only with this; without it\n'
                    '    # tachys panics at hydration.\n'
                    '    "leptos/lazy",\n',
                    '    # (Phase 10 A7, the 0.8 copy: Leptos 0.8.21 has no `lazy` feature.)\n',
                    what,
                )
            # The servers name `mf2::axum` (Phase 10 D3), whose line is `mf2`'s.
            if "0.9.0-beta" in text:
                sys.exit(f"{what}: a 0.9 requirement is left")
        manifest.write_text(text)
    if demo == "demo-csr":
        trunk = dst / "Trunk.toml"
        trunk.write_text(
            must_replace(
                trunk.read_text(),
                "--manifest-path ../../Cargo.toml",
                f"--manifest-path {ROOT / 'Cargo.toml'}",
                "demo-csr/Trunk.toml",
            )
        )
    print(dst)

# The harness, beside the copies.
e2e = OUT / "tools/e2e"
if e2e.exists():
    shutil.rmtree(e2e)
e2e.mkdir(parents=True)
for name in ["checks", "lib"]:
    shutil.copytree(ROOT / "tools/e2e" / name, e2e / name)
for name in ["run.mjs", "package.json"]:
    shutil.copy2(ROOT / "tools/e2e" / name, e2e / name)
(e2e / "node_modules").symlink_to((ROOT / "tools/e2e/node_modules").resolve())
print(e2e)
