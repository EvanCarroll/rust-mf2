//! The A/B baseline of D15 (plans/10 A5b): `b12-runtime` with the numeric
//! code over `fixed_decimal` (feature `fixed-decimal`). Reported, not
//! required to be clean: its panic paths are why the own buffer exists.
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
    b12_runtime_walk::run(&REGISTRY)
}
