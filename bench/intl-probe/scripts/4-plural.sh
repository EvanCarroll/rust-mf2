#!/usr/bin/env bash
# Item 4: the 15,041 CLDR samples through each engine's Intl.PluralRules.
exec "$(dirname "$0")/engines.sh" plural
