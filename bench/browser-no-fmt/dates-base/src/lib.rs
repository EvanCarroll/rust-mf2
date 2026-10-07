//! The base of the date harnesses (B4):
//! `b12-dates-walk` over `b12-runtime`'s registry — every core function, no
//! date function (an unannotated date/time is then a *Bad Operand*). Each
//! date harness is this plus its date handlers.
#![no_std]

use b12_dates_walk::STUB_HOST;
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
    b12_dates_walk::run(&REGISTRY, &STUB_HOST)
}
