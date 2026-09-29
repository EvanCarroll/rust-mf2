#!/usr/bin/env bash
# Phase 10 B2: the three demos' shipped files as `measure.sh` kept them for
# two runs (target/p10-b2/demos/LABEL/: demo-ssr's and demo-islands' `pkg/`,
# demo-csr's `dist/`), hashed and compared: every line `diff` prints is a
# file that differs between the runs.
#
#   bash probes/p10-b2/demo-hashes.sh [BASE [OTHER]]    # default: base b2
#
# Run from the root of B2's measurement tree. B2's runs predate the `pkg/`
# hashes in `measure.sh`; this reads what they kept. Outputs in
# target/p10-b2/logs/hashes/.
set -u
cd "$(dirname "$0")/../.."
out=target/p10-b2
a=${1:-base}
b=${2:-b2}
mkdir -p "$out/logs/hashes"
for label in "$a" "$b"; do
  ( cd "$out/demos/$label" && find . -type f -not -name measure.md -print0 | sort -z | xargs -0 sha256sum ) \
    >"$out/logs/hashes/$label.sha256"
done
echo "hashed: $(wc -l <"$out/logs/hashes/$a.sha256") files ($a), $(wc -l <"$out/logs/hashes/$b.sha256") ($b)"
if diff "$out/logs/hashes/$a.sha256" "$out/logs/hashes/$b.sha256"; then
  echo "every file byte-identical"
fi
