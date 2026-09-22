//! B12 for the `intl` client option with `mf2-fn-number`
//! (`plans/03-runtime.md` §5.3): `b12-runtime-fn-number-measure`'s registry
//! — `:number`, `:integer`, `:offset`, `:percent`, `:currency`, `:unit` and
//! unannotated numbers, localized — with the `intl` features, walked over
//! the stub number formatter. Built in its own cargo invocation.
#![no_std]

use mf2_fn_number::{CURRENCY, INTEGER, NUMBER, NUMBERS, OFFSET, PERCENT, UNIT};
use mf2_runtime::functions::STRING;
use mf2_runtime::{Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("currency", &CURRENCY),
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("percent", &PERCENT),
    ("string", &STRING),
    ("unit", &UNIT),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_numbers(&NUMBERS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_runtime_walk::run_intl(&REGISTRY)
}
