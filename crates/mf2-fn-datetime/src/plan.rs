//! The seam between the date semantics and a backend
//! (`plans/03-runtime.md` §5.2: "a backend only turns an instant plus a
//! style into text"): a resolved value becomes a [`Plan`] — the wall time to
//! show, the zone it is shown in, the options — and a [`Backend`] writes it.

use mf2_runtime::{
    Date, DateTime, DateTimeOptions, DateTimeRequest, Dir, FnContext, FormatError, Sink,
    SubPartSink, Time, ZoneOption,
};

use crate::zone::wall_ms;

/// What a backend formats: backend-independent, everything MF2 decided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct Plan<'p> {
    /// The date to show: the wall date in [`Plan::zone`].
    pub date: Date,
    /// The time to show: the wall time in [`Plan::zone`], to the millisecond.
    pub time: Time,
    /// The zone's UTC offset at that instant, in seconds east; `None`: not
    /// known — a floating value placed in a named zone the host has no data
    /// for.
    pub offset: Option<i32>,
    /// The zone the value is shown in: [`ZoneOption::Utc`], an offset or a
    /// named zone, never [`ZoneOption::Input`]. What `timeZoneStyle`
    /// names.
    pub zone: ZoneOption<'p>,
    /// The resolved options: `date` (fields and length) for `:datetime` and
    /// `:date`, `time` (precision) for `:datetime` and `:time`,
    /// `time_zone_style`, `hour12`, `calendar`. Their `time_zone` is the
    /// *option* (the inheritance record): a backend shows [`Plan::zone`].
    pub options: &'p DateTimeOptions<'p>,
}

impl<'p> Plan<'p> {
    /// The plan of `value`, which a date/time function resolved in the
    /// context `cx` (so it is in the zone its `options.time_zone` names,
    /// the context's when `None`).
    pub fn new(cx: &FnContext<'p>, value: &'p DateTime<'p>) -> Plan<'p> {
        let zone = match (value.zone, value.options.time_zone) {
            (Some(z), _) => ZoneOption::Named(z),
            (None, Some(z @ (ZoneOption::Utc | ZoneOption::Offset(_) | ZoneOption::Named(_)))) => z,
            // `timeZone=input`, none, or a kind this version does not know.
            (None, _) => cx.time_zone().as_option(),
        };
        Plan {
            date: value.date,
            time: value.time,
            offset: value.offset,
            zone,
            options: &value.options,
        }
    }

    /// The wall time, in milliseconds since the epoch, read as UTC.
    pub fn wall_ms(&self) -> i64 {
        wall_ms(&DateTime::floating(self.date, self.time))
    }

    /// The instant, in milliseconds since the epoch; `None` when the offset
    /// is not known.
    pub fn epoch_ms(&self) -> Option<i64> {
        self.offset.map(|o| self.wall_ms() - i64::from(o) * 1000)
    }

    /// The request for a host's date formatter (`Host::format_date_time`,
    /// `datetime-intl`): the instant in [`Plan::zone`]; without a known
    /// offset, the wall time read as UTC, shown in UTC (the same wall
    /// time, but the zone is lost).
    pub fn request(&self) -> DateTimeRequest<'p> {
        match self.epoch_ms() {
            Some(epoch_ms) => DateTimeRequest::new(epoch_ms, self.zone, self.options),
            None => DateTimeRequest::new(self.wall_ms(), ZoneOption::Utc, self.options),
        }
    }
}

/// A date/time backend: turns a [`Plan`] into text. The semantics —
/// operands, options, errors, zones — are done; a backend only formats.
/// Implemented by [`crate::Neutral`] (the stub), and by the `datetime-icu`
/// and `datetime-intl` backends behind features (`plans/11` A6).
///
/// Client-path code: no `core::fmt`, no panics, no allocation.
pub trait Backend: Sync {
    /// Whether `plan` can be formatted. `Err(e)`: the placeholder becomes
    /// a fallback value and `e` is reported (e.g. *Unsupported
    /// Operation* for a calendar the backend's data lacks). Called before
    /// `dir` and `format`.
    fn supports(&self, cx: &FnContext<'_>, plan: &Plan<'_>) -> Result<(), FormatError> {
        let _ = (cx, plan);
        Ok(())
    }

    /// Writes `plan` for `cx.locale()`.
    fn format(&self, cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink);

    /// Writes `plan`'s sub-parts (none by default).
    fn format_parts(&self, cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn SubPartSink) {
        let _ = (cx, plan, out);
    }

    /// The direction of the formatted text (the Default Bidi Strategy);
    /// `Ltr` by default.
    fn dir(&self, cx: &FnContext<'_>, plan: &Plan<'_>) -> Dir {
        let _ = (cx, plan);
        Dir::Ltr
    }
}
