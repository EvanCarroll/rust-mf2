//! B12 and B3: the registry of an
//! application whose corpus uses the whole localized numeric family —
//! `b12-runtime-fn-number`'s handlers plus `:currency` and `:unit`. B3 =
//! this − `b12-runtime-fn-number`.
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
    b12_runtime_walk::run(&REGISTRY)
}
