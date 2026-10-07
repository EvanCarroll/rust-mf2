//! B1′ for the `intl` client option: the
//! `intl` features on — `mf2-runtime`'s and `mf2-fn-number`'s, the crate
//! linked — but a corpus that formats no number, so the registry is
//! `b12-runtime-nonum`'s (`:string` alone) and the host the application
//! names has no number formatter. The module must be byte-identical in size
//! to `b12-runtime-nonum`'s (+0 B). Built in its own cargo invocation.
#![no_std]

use mf2_runtime::functions::STRING;
use mf2_runtime::{Function, Registry};

// Linked, not referenced by the registry.
use mf2_fn_number as _;

static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &STRING)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_runtime_walk::run(&REGISTRY)
}
