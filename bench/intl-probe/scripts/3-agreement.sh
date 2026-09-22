#!/usr/bin/env bash
# Item 3: P0.5's 100,000 cases (Rust vs Intl.NumberFormat; `rust` vs `intl`
# handlers), the edge cases, and the panel's locale-symbol cases compared
# engine vs engine (and vs the Rust registry, `loc-rust.json`).
exec "$(dirname "$0")/engines.sh" ecmaRaw ecmaHandlers edge loc
