#!/usr/bin/env bash
# snapshot.sh REV — prints the path of a tree holding commit REV's files with
# the working tree's bench/intl-probe on top (and the root workspace's
# `exclude` entry for it), under target/intl-probe/snap-<sha>/src. Builds
# there measure REV's runtime whatever crates/ holds now.
set -euo pipefail
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
rev=${1:?usage: snapshot.sh REV}
sha=$(git -C "$REPO" rev-parse --short "$rev")
dst="$REPO/target/intl-probe/snap-$sha/src"
if [ ! -f "$dst/.complete-$sha" ]; then
  rm -rf "$dst"
  mkdir -p "$dst"
  git -C "$REPO" archive "$sha" | tar -x -C "$dst"
  touch "$dst/.complete-$sha"
fi
# The probe as it is now (refreshed on every call; its target/ stays).
mkdir -p "$dst/bench/intl-probe"
tar -C "$REPO/bench/intl-probe" --exclude=./target -c . | tar -x -C "$dst/bench/intl-probe"
# Before Phase 4 A3 there is no mf2-fn-number: a stub keeps the probe's
# workspace resolvable (the `rust-loc` variant and feature `fn-number` do
# not build there; scripts/build.sh skips `rust-loc`).
if [ ! -f "$dst/crates/mf2-fn-number/Cargo.toml" ]; then
  mkdir -p "$dst/crates/mf2-fn-number/src"
  printf '[package]\nname = "mf2-fn-number"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n[workspace]\n' > "$dst/crates/mf2-fn-number/Cargo.toml"
  printf '//! Stub: this commit predates mf2-fn-number (scripts/snapshot.sh).\n#![no_std]\n' > "$dst/crates/mf2-fn-number/src/lib.rs"
fi
if ! grep -q '"bench/intl-probe"' "$dst/Cargo.toml"; then
  sed -i 's|^exclude = \[\(.*\)\]|exclude = [\1, "bench/intl-probe"]|' "$dst/Cargo.toml"
fi
echo "$dst"
