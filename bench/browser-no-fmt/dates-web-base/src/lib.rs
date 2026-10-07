//! The base of the `intl` harness (B4): `b12-dates-walk` over the core registry on the browser host
//! `mf2_host_web::HOST`, built through `wasm-bindgen`. `mf2-host-web`'s date
//! features are on, and `HOST` must link none of their glue (B1′): its
//! module imports no date function.
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
    b12_dates_walk::run(&REGISTRY, &mf2_host_web::HOST)
}
