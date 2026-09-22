//! B4 (`plans/06-size-and-perf.md` §3), `datetime-icu`: the date
//! functions over `Icu<AnyCalendar, NoZones>` — ICU4X 2.3 from the catalog's
//! `icu.blob` (any calendar, no zone styles). B4 = this − `b12-dates-base`. B12 is reported, not
//! gated: ICU4X brings its own `core::fmt` and panic paths, the feature's
//! documented cost; the date semantics are B12-clean (`b12-dates-neutral`).
#![no_std]

use b12_dates_walk::STUB_HOST;
use mf2_fn_datetime::DateTimeFunction;
use mf2_fn_datetime::icu::{AnyCalendar, Icu, NoZones};
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{Function, Registry};

type Backend = Icu<AnyCalendar, NoZones>;
static DATE: DateTimeFunction<Backend> = DateTimeFunction::date(Icu::NEW);
static DATETIME: DateTimeFunction<Backend> = DateTimeFunction::datetime(Icu::NEW);
static TIME: DateTimeFunction<Backend> = DateTimeFunction::time(Icu::NEW);
static DATES: DateTimeFunction<Backend> = DateTimeFunction::unannotated(Icu::NEW);
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
    b12_dates_walk::run(&REGISTRY, &STUB_HOST)
}
