# Sourced by every script of the probe (bench/intl-probe/scripts/*.sh).
#
#   REPO  the repository these scripts belong to (outputs go to $REPO/target/intl-probe)
#   SRC   the tree that is built: $REPO, or — with INTL_PROBE_REV=<rev> — a
#         snapshot of commit <rev>'s crates with this probe on top
#         (scripts/snapshot.sh), so a measurement pins the runtime it measures
#   PROBE $SRC/bench/intl-probe (cargo runs here)
#   OUT   $REPO/target/intl-probe
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
OUT="$REPO/target/intl-probe"
mkdir -p "$OUT"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
if [ -n "${INTL_PROBE_REV:-}" ]; then
  # Resolved once: HEAD may move while a script runs.
  INTL_PROBE_REV=$(git -C "$REPO" rev-parse --short "$INTL_PROBE_REV")
  export INTL_PROBE_REV
  SRC=$("$REPO/bench/intl-probe/scripts/snapshot.sh" "$INTL_PROBE_REV")
else
  SRC="$REPO"
fi
PROBE="$SRC/bench/intl-probe"
# What was built, for every report.
tree_note() {
  if [ -n "${INTL_PROBE_REV:-}" ]; then
    echo "crates/ at $INTL_PROBE_REV (snapshot ${SRC#"$REPO"/})"
  else
    local dirty
    dirty=$(git -C "$REPO" status --porcelain -- crates | wc -l)
    echo "crates/ of the working tree at $(git -C "$REPO" rev-parse --short HEAD) ($dirty changed paths under crates/)"
  fi
}
mhz() { grep -m1 MHz /proc/cpuinfo | sed 's/.*: *//'; }
