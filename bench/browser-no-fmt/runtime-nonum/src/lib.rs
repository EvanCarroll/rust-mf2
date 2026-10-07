//! B13 (closed world): `b12-runtime-walk` over a registry with `:string`
//! only. No numeric handler — resolution, digit options, rounding, the
//! plural evaluator — may be linked (`check.sh` greps for their symbols);
//! the difference `b12-runtime` − this is the core numeric semantics' share
//! of B1 (≤ 10 KB gz).
#![no_std]

use mf2_runtime::functions::STRING;
use mf2_runtime::{Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &STRING)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_runtime_walk::run(&REGISTRY)
}
