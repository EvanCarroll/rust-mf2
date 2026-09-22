//! B12 and B1 (runtime part): `b12-runtime-walk` over the registry of every
//! core function — the reader, the evaluator, `:string`, and `:number`,
//! `:integer`, `:offset` over the own digit buffer (D15).
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
