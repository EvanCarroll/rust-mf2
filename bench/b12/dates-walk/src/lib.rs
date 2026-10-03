//! The walk of the date harnesses (B4, B12, B13 for `mf2-fn-datetime`;
//! `plans/06-size-and-perf.md` §3): `b12-runtime-walk`'s client path —
//! `Catalog::new` on bytes from the host, every message formatted with
//! `Formatter::simple`, `write` and `parts` — with date/time arguments (an
//! instant, a floating value, an offset: host-chosen) beside the others, and
//! the formatting context's time zone chosen by the host (UTC, an offset, or
//! a named zone from the host's text), so every zone path stays reachable.
//! The host is the harness's: a stub whose `zone_offset` answers a number
//! from the host (`StubHost`, the native ICU4X harnesses — the browser's
//! zone data is `mf2-host-web`'s, measured by the web harnesses), or
//! `mf2-host-web`'s.
#![no_std]
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]

extern crate alloc;

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};
use mf2_catalog::{Catalog, MsgId};
use mf2_runtime::{
    Arg, Date, DateTime, ErrorSink, FormatContext, FormatError, Formatter, Host, Part, PartSink,
    Registry, Sink, SubPartSink, Time, TimeZone,
};

/// A host that prints no floats, whose zone offsets
/// are a number from the host: the host's own cost is not the runtime's.
pub struct StubHost;

impl Host for StubHost {
    fn f64_to_text<'b>(&self, _x: f64, _buf: &'b mut [u8; 32]) -> Option<&'b str> {
        None
    }

    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        let p = param(7) ^ (zone.len() as u64) ^ (epoch_ms as u64);
        (p & 1 == 0).then_some((p >> 1) as i32 % 86_400)
    }
}

/// The stub host, for the native harnesses.
pub static STUB_HOST: StubHost = StubHost;

/// Folds everything written into one number.
struct Fold(u64);

impl Fold {
    #[inline(never)]
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = self.0.rotate_left(5) ^ u64::from(x);
        }
    }
}

impl Sink for Fold {
    fn push_str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
}

impl SubPartSink for Fold {
    fn sub_part(&mut self, kind: &str, text: &str) {
        self.bytes(kind.as_bytes());
        self.bytes(text.as_bytes());
    }
}

impl ErrorSink for Fold {
    fn error(&mut self, e: FormatError) {
        // No `as` on a `#[non_exhaustive]` enum of another crate.
        let code = match e {
            FormatError::UnresolvedVariable => 1,
            FormatError::UnknownFunction => 2,
            FormatError::BadSelector => 3,
            FormatError::BadOperand => 4,
            FormatError::BadOption => 5,
            FormatError::BadVariantKey => 6,
            FormatError::UnsupportedOperation => 7,
            FormatError::MessageFunctionError => 8,
            FormatError::MissingFallbackVariant => 9,
            FormatError::MissingMessage => 10,
            FormatError::Malformed => 11,
            _ => 12,
        };
        self.0 = self.0.rotate_left(3) ^ code;
    }
}

impl PartSink for Fold {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(t) => self.bytes(t.as_bytes()),
            Part::BidiIsolation(i) => self.bytes(i.as_str().as_bytes()),
            Part::Expression(e) => {
                self.bytes(e.kind().as_bytes());
                self.bytes(e.locale().as_bytes());
                self.0 ^= e.dir() as u64;
                if let Some(id) = e.id() {
                    self.bytes(id.as_bytes());
                }
                e.write(self);
                e.sub_parts(self);
            }
            Part::Markup(m) => {
                self.0 ^= m.kind() as u64;
                self.bytes(m.name().as_bytes());
                if let Some(id) = m.id() {
                    self.bytes(id.as_bytes());
                }
                for (name, value) in m.options() {
                    self.bytes(name.as_bytes());
                    if let Some(s) = value.as_str() {
                        self.bytes(s.as_bytes());
                    }
                }
            }
            Part::Fallback(src) => src.write(self),
            _ => {}
        }
    }
}

/// A date/time from the host: an instant, a floating value, or an instant
/// at an offset.
fn date_time() -> Option<DateTime<'static>> {
    let p = param(4);
    match p % 3 {
        0 => DateTime::from_epoch_ms(param(5) as i64),
        1 => {
            let date = Date::from_days_since_epoch(param(5) as i64)?;
            let t = param(6);
            let time = Time::new(
                (t % 24) as u8,
                (t / 24 % 60) as u8,
                (t / 1440 % 60) as u8,
                (t / 86_400 % 1000) as u16,
            )?;
            Some(DateTime::floating(date, time))
        }
        _ => DateTime::from_epoch_ms(param(5) as i64)?.with_offset((p >> 2) as i32 % 86_400),
    }
}

/// The argument of kind `k` (chosen by the host) over the host's `text`.
fn arg<'a>(k: u64, text: &'a str, dt: Option<&'a DateTime<'a>>) -> Arg<'a> {
    match (k % 6, dt) {
        (0, _) => Arg::Str(text),
        (1, _) => Arg::Int(param(1) as i64),
        (2, _) => Arg::Float(f64::from_bits(param(2))),
        (3, _) => Arg::Decimal(text),
        (4, Some(dt)) => Arg::DateTime(dt),
        _ => Arg::Unset,
    }
}

/// Loads the host's catalog and formats every message with `registry`, on
/// `host`, in the zone the host chooses.
pub fn run(registry: &Registry, host: &'static dyn Host) -> u32 {
    let Some(bytes) = input(INPUT_CATALOG) else {
        return 1;
    };
    let Some(key) = input(INPUT_KEY) else {
        return 1;
    };
    let Ok(cat) = Catalog::new(bytes, param(0)) else {
        return 2;
    };
    let text = core::str::from_utf8(&key).unwrap_or("");
    let dt = date_time();
    // Four slots, each of a kind the host chooses.
    let kinds = param(3);
    let args = [
        arg(kinds, text, dt.as_ref()),
        arg(kinds >> 3, text, dt.as_ref()),
        arg(kinds >> 6, text, dt.as_ref()),
        arg(kinds >> 9, text, dt.as_ref()),
    ];
    let mut cx = FormatContext::new(host);
    cx.time_zone = match kinds >> 12 & 3 {
        0 => TimeZone::UTC,
        1 => TimeZone::offset((kinds >> 14) as i32 % 86_400).unwrap_or(TimeZone::UTC),
        _ => TimeZone::named(text).unwrap_or(TimeZone::UTC),
    };
    let f = Formatter::new(&cat, registry, &cx);
    let mut fold = Fold(0);
    for i in 0..=cat.message_count() {
        let Some(id) = MsgId::new(cat.chunk(), i) else {
            continue;
        };
        if let Some(s) = f.simple(id) {
            fold.bytes(s.as_bytes());
        }
        let mut errs = Fold(0);
        f.write(id, &args, &mut fold, &mut errs);
        f.parts(id, &args, &mut fold, &mut errs);
        fold.0 ^= errs.0;
    }
    sink(fold.0);
    sink_bytes(&key);
    sink_bytes(cat.as_bytes());
    0
}
