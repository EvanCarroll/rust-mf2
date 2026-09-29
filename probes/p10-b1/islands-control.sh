#!/usr/bin/env bash
# Phase 10 B1: what `format!` itself costs demo-islands, whose client calls
# it nowhere else — the control for `{:?}` on a `TrArgs` there.
set -u
cd "$(dirname "$0")/../.."
out=$(pwd)/target/p10-b1
logs=$out/logs/display-debug
export CARGO_NET_OFFLINE=true
f=examples/demo-islands/src/lib.rs
anchor='    leptos_mf2::install(demo_islands_i18n::setup());'
run() {
  local label=$1 stmt=$2
  python3 - "$f" "$anchor" "$stmt" <<'PY'
import sys
path, anchor, stmt = sys.argv[1:]
text = open(path).read()
assert text.count(anchor + "\n") == 1
text = text.replace(anchor + "\n", anchor + "\n    // B1 control (a working-tree edit, never committed)\n    {\n        " + stmt + "\n    }\n")
open(path, "w").write(text)
PY
  (cd examples/demo-islands && CARGO_BUILD_JOBS=2 cargo leptos build --release --frontend-only --cargo-offline) >"$logs/islands-$label.log" 2>&1
  echo "built $label rc=$?"
  git checkout -q -- "$f"
  local dest=$out/demos/case-$label/demo-islands
  rm -rf "$dest"; mkdir -p "$dest"
  cp -a examples/demo-islands/target/site/pkg "$dest/out"
  node probes/p10-names/measure-demo.mjs "$dest/out" "demo-islands $label" | tee "$dest/measure.md"
}
run format-string 'let x = std::hint::black_box(String::from("a")); std::hint::black_box(format!("{}", x));'
run debug-string 'let x = std::hint::black_box(String::from("a")); std::hint::black_box(format!("{:?}", x));'
run debug-tr 'let x = std::hint::black_box(demo_islands_i18n::tr!("app-title")); std::hint::black_box(format!("{:?}", x));'
