//! B1′ and B13: `datetime` on — with
//! both backends' features — but unused by the corpus. The crate is a
//! dependency and named here, but the registry and the walk are
//! `b12-runtime`'s, so the module must be byte-identical in size to it
//! (+0 B) and link no date code.
#![no_std]

use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

// Linked, not referenced by the registry.
use mf2_fn_datetime as _;

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
