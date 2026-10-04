//! What the runtime asks of its platform:
//! the shortest text of a float (no float-printing code in the
//! wasm), for dates the UTC offset of a named time zone and —
//! with the `intl` date formatter — a date formatter, and — `intl` — a number formatter
//! with plural rules. `mf2-host-std` implements it natively and for
//! `wasm32-wasip1`, `mf2-host-web` in the browser.
//!
//! Canonical equivalence is not among them: the runtime answers it itself,
//! from the map its catalog carries (`crate::nfc_equivalent`, `plan/01`
//! §4.3), so no host needs normalization tables.

use crate::datetime::DateTimeRequest;
use crate::number::{NumberOut, NumberRequest};
use crate::plural::Category;
use crate::sink::Sink;

/// The platform services the runtime needs.
pub trait Host: Sync {
    /// The shortest decimal text that round-trips the finite `x`, written
    /// into `buf`: any form `number-literal` accepts, with an optional `+` in
    /// the exponent (`ryu`'s `4.2`, `1e21`, `1.5e-7` and JavaScript's
    /// `String(x)` both qualify). `None` if the host cannot produce it.
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str>;

    /// The UTC offset, in seconds east, of the IANA time zone `zone` at the
    /// instant `epoch_ms` (milliseconds since the epoch). `None`: the host
    /// has no zone data, or knows no such zone (the default) — a date/time
    /// function that must convert an instant to a named zone then reports
    /// *Bad Option* and a fallback value (datetime.md allows it; a wall time
    /// in the wrong zone would be worse).
    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        let _ = (zone, epoch_ms);
        None
    }

    /// The `intl` date formatter: writes `request` formatted by the host's date
    /// formatter (the browser's `Intl.DateTimeFormat`) for `locale`, and
    /// returns `true`; `false` (the default) when the host has none.
    fn format_date_time(
        &self,
        locale: &str,
        request: &DateTimeRequest<'_>,
        out: &mut dyn Sink,
    ) -> bool {
        let _ = (locale, request, out);
        false
    }

    /// `intl`: the host's number formatter (in the browser `Intl.NumberFormat`
    /// and `Intl.PluralRules`, when the engine has `Intl.NumberFormat` v3);
    /// `None` (the default) when it has none — the numeric functions of an
    /// `intl` client then show exact digits and report *Unsupported
    /// Operation*.
    fn numbers(&self) -> Option<&dyn NumberFormatter> {
        None
    }
}

/// A number formatter for the `intl` option ([`Host::numbers`]):
/// the final "value + resolved options →
/// text" step and the plural category, where the numeric functions keep
/// MF2's semantics in Rust. `mf2-host-web` implements it with `Intl`.
pub trait NumberFormatter: Sync {
    /// Writes `request` formatted for `locale` — or, when `request.neutral`,
    /// in neutral symbols — as text or sub-parts, and returns `true`;
    /// `false` when it cannot (nothing written).
    fn format(&self, locale: &str, request: &NumberRequest<'_>, out: NumberOut<'_>) -> bool;

    /// The plural category of `request.value` under its digit options and
    /// plural type (`request.ordinal`) for `locale`; `None` when it cannot.
    fn plural(&self, locale: &str, request: &NumberRequest<'_>) -> Option<Category>;
}
