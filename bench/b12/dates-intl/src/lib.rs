//! B12 and B4 (`plans/06-size-and-perf.md` §3), `intl`: the date
//! functions over the `Intl` backend on `mf2_host_web::INTL_HOST` —
//! `Intl.DateTimeFormat` for the text, the browser's zone data for
//! `zone_offset` — built through `wasm-bindgen`. B4 = this −
//! `b12-dates-web-base`: wasm ≤ 6 KB gz (the semantics included) and JS
//! glue ≤ 1 KB gz.
#![no_std]

use mf2_fn_datetime::{DateTimeFunction, Intl};
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

static DATE: DateTimeFunction<Intl> = DateTimeFunction::date(Intl);
static DATETIME: DateTimeFunction<Intl> = DateTimeFunction::datetime(Intl);
static TIME: DateTimeFunction<Intl> = DateTimeFunction::time(Intl);
static DATES: DateTimeFunction<Intl> = DateTimeFunction::unannotated(Intl);
static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("date", &DATE),
    ("datetime", &DATETIME),
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("string", &STRING),
    ("time", &TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&DATES);

/// Formats every message of the host's catalog.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    b12_dates_walk::run(&REGISTRY, &mf2_host_web::INTL_HOST)
}
