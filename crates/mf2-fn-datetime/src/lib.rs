//! `mf2-fn-datetime` — the MessageFormat 2 date/time functions of mf2-two
//! (`functions/datetime.md`; `plans/03-runtime.md` §2.7, §5.2, §6):
//! `:datetime`, `:date`, `:time`, and the handler that formats unannotated
//! date/time values. The semantics — operands, options, errors, time zones
//! — are here, once; a [`Backend`] only turns the result, a [`Plan`], into
//! text. With no backend feature on, the handlers format through
//! [`Neutral`], a deterministic locale-independent stub (ISO 8601 pieces).
//!
//! ```
//! # use mf2::{FormatContext, Formatter, Registry};
//! use mf2_fn_datetime::{DATE, DATES, DATETIME, TIME};
//!
//! static FUNCTIONS: [(&str, &dyn mf2::Function); 3] =
//!     [("date", &DATE), ("datetime", &DATETIME), ("time", &TIME)];
//! static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&DATES);
//! static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);
//!
//! let m = mf2::compile_str("{|2006-01-02T15:04:06| :datetime timePrecision=second}", "en").unwrap();
//! let mut out = String::new();
//! let mut errors = Vec::new();
//! Formatter::new(&m.catalog, &REGISTRY, &CX).write(mf2::Compiled::ID, &[], &mut out, &mut errors);
//! assert_eq!(out, "2006-01-02 15:04:06");
//! assert!(errors.is_empty());
//! ```
//!
//! # The handlers
//!
//! | Static | Registry name | What |
//! |---|---|---|
//! | [`DATETIME`] | `datetime` | date and time; `dateFields` (`year-month-day`), `dateLength` (`medium`), `timePrecision` (`minute`), `timeZoneStyle` |
//! | [`DATE`] | `date` | the date; `fields` (`year-month-day`), `length` (`medium`) |
//! | [`TIME`] | `time` | the time; `precision` (`minute`), `timeZoneStyle` |
//! | [`DATES`] | — (`Registry::with_dates`) | an unannotated date/time, as `:datetime` with its defaults |
//!
//! All three functions take the override options `timeZone` and `calendar`;
//! `:datetime` and `:time` also `hour12`. Closed world (B13): a registry
//! names only the handlers its corpus uses, and an unused one is never
//! linked. A handler over another backend: [`DateTimeFunction::datetime`]
//! and its siblings.
//!
//! # Semantics, and the choices the spec leaves open
//!
//! * **Operands.** A date/time value — an [`Arg::DateTime`], or what a
//!   date/time function resolved, with its options — or an application
//!   value's `CustomValue::as_date_time`, or a string that is a *date/time
//!   literal value*: a literal, a string argument or an application value's
//!   `as_str` ([`parse_literal`]). A literal must match the spec's regular
//!   expression as a whole and name a day that exists; the spec's MAY —
//!   other ISO 8601 forms — is not taken, so what parses is exactly the
//!   regex (`-00:00` is offset 0). Without a time it is 00:00:00, without
//!   an offset floating. Anything else (numbers, booleans, a fallback
//!   value, no operand) is *Bad Operand* and a fallback value.
//! * **A `:date` value as a `:time` operand** (and the reverse, which the
//!   spec says MAY be a *Bad Operand*): accepted. A resolved value keeps the
//!   whole date/time of its operand, so `{$d :time}` over `$d = {|…T15:04:06|
//!   :date}` shows 15:04; a date literal's time is 00:00.
//! * **Options.** Each function takes exactly the options datetime.md lists
//!   for it; any other option is ignored (so `hour12` and `timeZoneStyle`
//!   on `:date`). The non-override options must be literals: a variable is
//!   *Bad Option* and the option is ignored. A value that is not one the
//!   option takes is *Bad Option*, ignored. Values compare exactly
//!   (case-sensitive). An override option may come from a variable whose
//!   value is a string (or an application value with `as_str`); any other
//!   value is *Bad Option*.
//! * **Override options.** `timeZone`: `input`, `UTC`, an RFC 3339
//!   `time-numoffset` (`±hh:mm`, hour 00–23), or an RFC 9557
//!   `time-zone-name` ([`mf2_runtime::is_zone_name`]; any well-formed name —
//!   whether a zone of that name exists is the host's knowledge). `hour12`:
//!   `true` / `false`. `calendar`: a well-formed `uvalue`, `3*8alphanum
//!   *("-" 3*8alphanum)` in its BCP 47 spelling (UTS 35 also admits `_`,
//!   which `Intl.DateTimeFormat` rejects); whether the calendar is known is
//!   the backend's to report. They are inherited from a date/time operand
//!   (an argument may carry them too) and the expression's own take
//!   priority; the operand's other options are not inherited: over `.local
//!   $d = {|2006-01-02| :date length=long}`, `{$d}` formats `$d` as it was
//!   resolved (long), `{$d :date}` is a new `:date` (medium). A `:date`
//!   value keeps an `hour12` it inherited, for a later `:time`.
//! * **Resolved value.** A [`Value::DateTime`] whose `options` hold what
//!   resolved: `date` for `:datetime` and `:date`, `time` for `:datetime`
//!   and `:time`, `time_zone_style`, `hour12`, `calendar`, and `time_zone`
//!   — the zone the value is now in (`None`: the formatting context's). Not
//!   selectable: a selector on it is *Bad Selector* (the runtime's). Part
//!   kind `datetime`, direction `Ltr` for the neutral backend.
//! * **Time zones** (`zone`'s `place`). The default of `timeZone` is the
//!   formatting context's zone ([`FnContext::time_zone`]). `input` is the
//!   operand's own zone; on a floating operand it is *Bad Operand* and the
//!   context's zone is used (and recorded, so a later expression inheriting
//!   it does not report it again). A floating value takes the target zone
//!   without conversion; a value with an offset or a zone is converted to
//!   a different target: to UTC or an offset by arithmetic, to a named zone
//!   through [`Host::zone_offset`]. A floating wall time placed in a named
//!   zone gets its instant from a bracketing search over `zone_offset`
//!   (java.time's / Temporal's `compatible` choice: in an overlap the
//!   earlier instant, in a gap the wall time moves forward by the gap).
//! * **No zone data** (`Host::zone_offset` answers `None`, the default).
//!   Converting a value with an offset to a named zone, or a value in a
//!   named zone whose offset is unknown to any other zone, cannot be done:
//!   *Bad Option* and a fallback value, the alternative datetime.md allows —
//!   not a wall time shown in the wrong zone, and not *Unsupported
//!   Operation*, since the conversion itself is what the spec asks for. This
//!   includes the formatting context's default zone, which is the
//!   `timeZone` option's resolved value when the expression sets none: a
//!   context in a named zone needs a host with zone data for instants
//!   (`plans/11` owner decision 1). Placing a *floating* value in a named
//!   zone needs no conversion, so it is no error: the wall time shows, and
//!   the zone's offset stays unknown (the neutral backend then names the
//!   zone for `timeZoneStyle`). A converted value past `Date`'s year limit
//!   is *Bad Operand*.
//! * **Unannotated** date/time values ([`DATES`], `Registry::with_dates`):
//!   formatted as `:datetime` with no options (its own override options
//!   apply); if that resolution reports an error, the placeholder is a
//!   fallback value with that error.
//!
//! Client-path code: `no_std`, `forbid(unsafe_code)`, no `core::fmt` use,
//! no panicking operation, no allocation (B12, `plans/05-tooling.md` §8).
//!
//! [`Arg::DateTime`]: mf2_runtime::Arg::DateTime
//! [`Value::DateTime`]: mf2_runtime::Value::DateTime
//! [`FnContext::time_zone`]: mf2_runtime::FnContext::time_zone
//! [`Host::zone_offset`]: mf2_runtime::Host::zone_offset

#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod function;
mod literal;
mod neutral;
mod options;
mod plan;
mod zone;

pub use function::{DateTimeFunction, operand};
pub use literal::parse_literal;
pub use neutral::Neutral;
pub use plan::{Backend, Plan};

/// The backend the statics format with: [`Neutral`] while no backend
/// feature is on (`plans/11` A6 adds `datetime-icu` and `datetime-intl`).
pub type DefaultBackend = Neutral;

const DEFAULT_BACKEND: DefaultBackend = Neutral;

/// `:datetime`.
pub static DATETIME: DateTimeFunction = DateTimeFunction::datetime(DEFAULT_BACKEND);

/// `:date`.
pub static DATE: DateTimeFunction = DateTimeFunction::date(DEFAULT_BACKEND);

/// `:time`.
pub static TIME: DateTimeFunction = DateTimeFunction::time(DEFAULT_BACKEND);

/// Unannotated date/time values, for `Registry::with_dates`: as
/// `:datetime` with its defaults.
pub static DATES: DateTimeFunction = DateTimeFunction::unannotated(DEFAULT_BACKEND);
