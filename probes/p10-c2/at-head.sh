#!/usr/bin/env bash
# Phase 10 C2: runs a command with the crates' sources as HEAD has them, and
# puts C2's back afterwards, whatever happens: what `crates/` holds that git
# sees as changed or new is kept aside (`target/p10-c2/at-head/c2-src.tar`),
# HEAD's version of each changed file written in its place and each new file
# removed; then everything is restored with fresh modification times, so
# that the next build compiles C2's sources again. Everything outside
# `crates/` (a harness, a probe) stays as it is.
#
#   bash probes/p10-c2/at-head.sh CMD [ARG…]
#
# Run from the main tree, one build at a time.
set -eu
cd "$(dirname "$0")/../.."
keep=target/p10-c2/at-head
mkdir -p "$keep"
changed=$(git diff --name-only HEAD -- crates)
added=$(git ls-files --others --exclude-standard -- crates)
# shellcheck disable=SC2086
tar -cf "$keep/c2-src.tar" $changed $added
restore() { tar -xmf "$keep/c2-src.tar"; }
trap restore EXIT
for f in $changed; do git show "HEAD:$f" >"$f"; done
# shellcheck disable=SC2086
rm -f $added
"$@"
