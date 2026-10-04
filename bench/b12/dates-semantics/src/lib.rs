//! B12 and B4 (`plans/06-size-and-perf.md` §3): the date semantics every
//! backend needs — operands and literals, options and inheritance, zones
//! (the host's offsets, the gap/overlap search), the resolved value and its
//! plan — under `:datetime`, `:date` and `:time`, over a backend that writes one byte folded from everything the
//! plan holds (so no decision is dead code, and no formatting is measured).
//! Semantics = this − `b12-dates-base` (≤ 3.5 KB gz of B4).
#![no_std]

use b12_dates_walk::STUB_HOST;
use mf2_fn_datetime::{Backend, DateTimeFunction, Plan};
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{FnContext, Function, Registry, Sink, ZoneOption};

/// Writes one byte of the plan.
struct OneByte;

impl Backend for OneByte {
    fn format(&self, _cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
        let o = plan.options;
        let zone = match plan.zone {
            ZoneOption::Utc => 1,
            ZoneOption::Offset(s) => i64::from(s),
            ZoneOption::Named(n) => n.len() as i64,
            _ => 0,
        };
        let h = plan.wall_ms()
            ^ i64::from(plan.offset.unwrap_or(1))
            ^ zone
            ^ o.date.map_or(0, |d| d.fields as i64 * 3 + d.length as i64)
            ^ o.time.map_or(0, |p| p as i64 * 5)
            ^ o.time_zone_style.map_or(0, |z| z as i64 * 7)
            ^ o.hour12.map_or(0, |b| i64::from(b) * 11)
            ^ o.calendar.map_or(0, |c| c.len() as i64);
        out.push_str(if h & 1 == 0 { "0" } else { "1" });
    }
}

static DATE: DateTimeFunction<OneByte> = DateTimeFunction::date(OneByte);
static DATETIME: DateTimeFunction<OneByte> = DateTimeFunction::datetime(OneByte);
static TIME: DateTimeFunction<OneByte> = DateTimeFunction::time(OneByte);
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
