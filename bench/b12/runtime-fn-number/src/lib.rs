//! B12 and B2 (`fn-number` on and used, `plans/06-size-and-perf.md` §3):
//! `b12-runtime-walk` over the registry an application whose corpus uses
//! every function of the localized decimal family would generate —
//! `mf2-fn-number`'s `:number`, `:integer`, `:offset`, `:percent`, and its
//! handler for unannotated numbers. B2 = this − `b12-runtime`.
#![no_std]

use mf2_fn_number::{INTEGER, NUMBER, NUMBERS, OFFSET, PERCENT};
use mf2_runtime::functions::STRING;
use mf2_runtime::{Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("percent", &PERCENT),
    ("string", &STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_numbers(&NUMBERS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_runtime_walk::run(&REGISTRY)
}
