#!/usr/bin/env bash
# Where a test run's time went: the test binaries of a `cargo test` log by
# their own run time, longest first, then the total. The log is `cargo xtask
# ci`'s output (target/p10-checks/LABEL/ci.log) or `cargo test`'s.
#
#   bash tools/checks/test-times.sh LOG [ROWS]
#
# The times are libtest's "finished in" figures: the tests running, not the
# build. The tests of one binary run in parallel; the binaries run one after
# another, so the total is what the run waited for.
set -u
log=${1:?usage: test-times.sh LOG [ROWS]}
rows=${2:-12}
[ -f "$log" ] || { echo "no $log" >&2; exit 2; }
times() {
  awk '/^ +Running / { sub(/^ +Running /, ""); name = $0; next }
       /^ +Doc-tests / { sub(/^ +/, ""); name = $0; next }
       /^test result:/ && match($0, /finished in [0-9.]+s/) {
         t = substr($0, RSTART + 12, RLENGTH - 13); n = 0
         for (i = 1; i < NF; i++) if ($(i + 1) ~ /^passed/) n = $i
         printf "%8.1f s  %4d tests  %s\n", t, n, name }' "$log"
}
times | sort -rn | head -n "$rows"
times | awk '{ total += $1 } END { printf "%8.1f s  total, %d test binaries\n", total, NR }'
