//! B1′ (`plans/06-size-and-perf.md` §3): `fn-number` on but unused. The
//! crate is a dependency and named here, but the registry is `b12-runtime`'s
//! — what an application's generated registry is when its corpus formats no
//! localized number — so the module must be byte-identical in size to
//! `b12-runtime`'s (+0 B).
#![no_std]

use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

// Linked, not referenced by the registry.
use mf2_fn_number as _;

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
