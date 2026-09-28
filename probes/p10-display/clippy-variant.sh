#!/usr/bin/env bash
# A5: the variant's lint status. `cargo xtask ci`'s clippy steps that compile
# leptos-mf2 (xtask/src/ci.rs), then two it does not have: the Leptos-free
# core (`--no-default-features`, where display.rs's empty `ambient` compiles)
# and the variant's render test. Run in the nested worktree target/a5-dev
# (branch p10-a5-dev, 70e0b27, clean). Each step's command and exit code go to
# target/a5/clippy-variant.log; its output to clippy-variant/<n>.txt.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dev="$here/../a5-dev"
log="$here/clippy-variant.log"
out="$here/clippy-variant"
mkdir -p "$out"
: > "$log"
cd "$dev"
echo "tree: $(git log -1 --format='%h %s') ($(git status --short | wc -l) changed files) $(date -Is)" >> "$log"
export CARGO_BUILD_JOBS=2
n=0
step() {
  n=$((n + 1))
  echo "[$n] cargo $* (start $(date -Is), load $(cut -d' ' -f1 /proc/loadavg))" >> "$log"
  nice -n 10 cargo "$@" > "$out/$n.txt" 2>&1
  echo "[$n] exit $? ($(date -Is))" >> "$log"
}
step clippy --workspace --all-targets -- -D warnings
for f in hydrate hydrate,static-locale csr csr,static-locale hydrate,mark-fallback-lang \
  csr,static-locale,mark-fallback-lang hydrate,fn-datetime \
  hydrate,fn-datetime,static-locale,mark-fallback-lang csr,fn-datetime; do
  step clippy --target wasm32-unknown-unknown -p leptos-mf2 --features "$f" -- -D warnings
done
step clippy -p leptos-mf2 --features ssr,mark-fallback-lang --all-targets -- -D warnings
step clippy -p leptos-mf2 --no-default-features --lib -- -D warnings
step clippy -p leptos-mf2 --no-default-features --lib --target wasm32-unknown-unknown -- -D warnings
step test -p leptos-mf2 --features ssr --test render
echo "done $(date -Is)" >> "$log"
