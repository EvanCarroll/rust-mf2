//! B12 and B13 for the `intl` client option (`plans/03-runtime.md` §2.7,
//! §5.3): `b12-runtime`'s registry — `:string` and the core's `:number`,
//! `:integer`, `:offset` — with `mf2-runtime`'s `intl` feature, walked over
//! the stub number formatter (`b12_runtime_walk::run_intl`). The display,
//! `:integer`'s rounding and the plural category come from the formatter:
//! the Rust rounding, digit output and plural evaluator must not be linked
//! (`check.sh`, B13). Built in its own cargo invocation.
#![no_std]

use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("string", &STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_runtime_walk::run_intl(&REGISTRY)
}
