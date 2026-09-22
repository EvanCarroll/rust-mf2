#!/usr/bin/env bash
# Every step: the modules (item 1), the data, items 2–6.
set -euo pipefail
here="$(dirname "$0")"
"$here/build.sh"
"$here/data.sh"
"$here/5-floor.sh"
"$here/6-l4.sh"
"$here/4-plural.sh"
"$here/3-agreement.sh"
"$here/2-speed.sh"
