//! B12 and B4 (`plans/06-size-and-perf.md` §3): the date semantics every
//! backend needs — `:datetime`, `:date` and `:time` over the neutral stub
//! backend. Semantics = this − `b12-dates-base`
//! (≤ 3.5 KB gz of B4).
#![no_std]

use b12_dates_walk::STUB_HOST;
use mf2_fn_datetime::{DateTimeFunction, Neutral};
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

type Backend = Neutral;
static DATE: DateTimeFunction<Backend> = DateTimeFunction::date(Neutral);
static DATETIME: DateTimeFunction<Backend> = DateTimeFunction::datetime(Neutral);
static TIME: DateTimeFunction<Backend> = DateTimeFunction::time(Neutral);
static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("date", &DATE),
    ("datetime", &DATETIME),
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("string", &STRING),
    ("time", &TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_dates_walk::run(&REGISTRY, &STUB_HOST)
}
