#!/usr/bin/env bash
# Phase 10 B2's refusal of `native` beside `csr` (plans/19 §3), met the way
# a 1.x application could meet it (B1's review fixes, item 9): one
# workspace holding a client-only web crate that names `csr` unconditionally
# on `leptos-mf2` and `mf2`, as `examples/demo-csr` does, and a command-line
# tool on the `mf2-native` shim. Each crate alone compiles; `cargo check
# --workspace` (rust-analyzer's default check) unifies `mf2`'s features.
#
#   bash probes/p10-b2/unify.sh
#
# Writes the workspace to target/p10-b2/unify/ (the main tree's Cargo.lock
# copied in, offline) and prints each check's exit status and every error
# it reports; the logs stay there.
set -u
cd "$(dirname "$0")/../.."
root=$(pwd)
ws=$root/target/p10-b2/unify
rm -rf "$ws"
mkdir -p "$ws/web/src" "$ws/cli/src"
cat >"$ws/Cargo.toml" <<'EOF'
[workspace]
resolver = "3"
members = ["web", "cli"]
EOF
cat >"$ws/web/Cargo.toml" <<EOF
[package]
name = "web"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
leptos-mf2 = { path = "$root/crates/leptos-mf2", features = ["csr"] }
mf2 = { path = "$root/crates/mf2", features = ["csr"] }
EOF
echo 'pub use leptos_mf2::{Tr, install};' >"$ws/web/src/lib.rs"
cat >"$ws/cli/Cargo.toml" <<EOF
[package]
name = "cli"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
mf2-native = { path = "$root/crates/mf2-native" }
EOF
echo 'fn main() { let _ = mf2_native::LocaleSource::System; }' >"$ws/cli/src/main.rs"
cp "$root/Cargo.lock" "$ws/Cargo.lock"
cd "$ws"
check() {
  local name=$1; shift
  CARGO_BUILD_JOBS=3 cargo check "$@" --offline --color never >"$name.log" 2>&1
  echo "== cargo check $* → exit $?"
  grep -E '^error' "$name.log" | sort | uniq -c
}
check web -p web
check cli -p cli
check workspace --workspace
