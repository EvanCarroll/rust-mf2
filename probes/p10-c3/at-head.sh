#!/usr/bin/env bash
# Phase 10 C3: runs a command with the sources as HEAD has them, and puts
# C3's back afterwards, whatever happens (C2's `probes/p10-c2/at-head.sh`,
# which swaps `crates/`, with `examples/demo-csr/i18n/src/lib.rs` swapped
# too: C3 gives that setup the build's table, which HEAD's `mf2` lacks).
# What those paths hold that git sees as changed or new is kept aside
# (`target/p10-c3/at-head/c3-src.tar`), HEAD's version of each changed file
# written in its place and each new file removed; then everything is
# restored with fresh modification times, so that the next build compiles
# C3's sources again.
#
#   bash probes/p10-c3/at-head.sh CMD [ARG…]
#
# Run from the main tree, one build at a time.
set -eu
cd "$(dirname "$0")/../.."
keep=target/p10-c3/at-head
mkdir -p "$keep"
paths="crates examples/demo-csr/i18n/src/lib.rs"
# shellcheck disable=SC2086
changed=$(git diff --name-only HEAD -- $paths)
# shellcheck disable=SC2086
added=$(git ls-files --others --exclude-standard -- $paths)
# shellcheck disable=SC2086
tar -cf "$keep/c3-src.tar" $changed $added
restore() { tar -xmf "$keep/c3-src.tar"; }
trap restore EXIT
for f in $changed; do git show "HEAD:$f" >"$f"; done
# shellcheck disable=SC2086
rm -f $added
"$@"
