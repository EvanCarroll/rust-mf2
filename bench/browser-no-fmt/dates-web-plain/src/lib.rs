//! B1′ for `mf2-host-web`'s date features
//! (`time-zones`, `datetime-intl`): `b12-dates-web-base`'s source, built in
//! its own cargo invocation without them. `b12-dates-web-base` (built with
//! them on) must be the same size, wasm and JS: an application that names
//! `HOST` pays nothing for the date hosts it does not name.
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
