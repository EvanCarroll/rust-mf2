//! `mf2-fn-datetime` — the MessageFormat 2 date/time functions of Rust MF2:
//! `:datetime`, `:date`, `:time`, and the handler that formats unannotated
//! date/time values. The semantics — operands, options, errors, time zones
//! — are here, once; a [`Backend`] only turns the result, a [`Plan`], into
//! text:
//!
//! One feature per side and formatter; a build formats with the strongest
//! of its own side's (ICU4X, then `Intl`, then the stub), and the other
//! side's features change nothing in it:
//!
//! | Feature | Side | [`DefaultBackend`] (the statics') |
//! |---|---|---|
//! | none | both | [`Neutral`], a deterministic locale-independent stub (ISO 8601 pieces) |
//! | `std-icu` | native | `icu::Icu`: ICU4X over the catalog's `icu.blob` LOCALE entry (narrower variants: `Icu<GregorianOnly, NoZones>` …) |
//! | `web-icu` | `wasm32-unknown-unknown` | `icu::Icu`, the same, so the browser writes the server's bytes |
//! | `web-intl` | `wasm32-unknown-unknown` | `Intl`: `Host::format_date_time` (the browser's `Intl.DateTimeFormat` through `mf2-host-web`'s `INTL_HOST`), unless `web-icu` is on |
//! | `compiled-data` | native | no default: `icu::Compiled`, ICU4X's compiled data, for a registry written by hand (implies `std-icu`) |
//!
//! ```
//! use mf2_fn_datetime::{DateTimeFunction, Neutral};
//! use mf2_runtime::{Function, Registry};
//!
//! // `DATETIME`, `DATE`, `TIME`, `DATES` are these over the default backend.
//! static DATETIME: DateTimeFunction<Neutral> = DateTimeFunction::datetime(Neutral);
//! static DATES: DateTimeFunction<Neutral> = DateTimeFunction::unannotated(Neutral);
//! static FUNCTIONS: [(&str, &dyn Function); 1] = [("datetime", &DATETIME)];
//! static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&DATES);
//! ```
//!
//! A message formatted with that registry: the `mf2` facade's front page,
//! which reaches this crate as `mf2::fn_datetime`.
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
//! `:datetime` and `:time` also `hour12`. Closed world: a registry
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
//!   *Bad Option* and a fallback value, the alternative the specification
//!   allows —
//!   not a wall time shown in the wrong zone, and not *Unsupported
//!   Operation*, since the conversion itself is what the spec asks for. This
//!   includes the formatting context's default zone, which is the
//!   `timeZone` option's resolved value when the expression sets none: a
//!   context in a named zone needs a host with zone data for instants.
//!   Placing a *floating* value in a named
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
//! no panicking operation, no allocation —
//! the semantics, the neutral and the `Intl` backends. The ICU4X backend
//! allocates (the blob's provider) and links ICU4X's own `core::fmt` and
//! panic paths: that is `web-icu`'s cost in client size.
//!
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.
//! An application reaches this crate through
//! [`mf2`](https://docs.rs/mf2), as `mf2::fn_datetime` (feature `datetime`, which every date formatter of
//! `mf2` turns on).
//!
//! [`Arg::DateTime`]: mf2_runtime::Arg::DateTime
//! [`Value::DateTime`]: mf2_runtime::Value::DateTime
//! [`FnContext::time_zone`]: mf2_runtime::FnContext::time_zone
//! [`Host::zone_offset`]: mf2_runtime::Host::zone_offset

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

#[cfg(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
))]
extern crate alloc;

mod function;
#[cfg(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
))]
pub mod icu;
#[cfg(feature = "web-intl")]
mod intl;
mod literal;
mod neutral;
mod options;
mod plan;
mod zone;

pub use function::{DateTimeFunction, operand};
// The build's reading of a literal's options (`mf2-locale-data`'s `icu.blob`).
#[doc(hidden)]
pub use function::literal_options;
#[cfg(feature = "web-intl")]
pub use intl::Intl;
pub use literal::parse_literal;
pub use neutral::Neutral;
pub use plan::{Backend, Plan};

// One formatter per build, the strongest of its side's (plan/08 §3.2):
// ICU4X, then `Intl`, then the neutral stub. In the browser
// (`wasm32-unknown-unknown`) the side's features are `web-icu` and
// `web-intl`; everywhere else `std-icu`. The other side's features change
// nothing in this build.

/// The backend the statics format with: the strongest of this build's side.
/// In the browser [`icu::Icu`] with `web-icu`, else [`Intl`] with `web-intl`,
/// else [`Neutral`]; elsewhere [`icu::Icu`] with `std-icu`, else [`Neutral`].
/// [`icu::Icu`] reads the catalog's `icu.blob`.
#[cfg(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
))]
pub type DefaultBackend = icu::Icu;

/// The backend the statics format with (see the ICU4X build).
#[cfg(all(
    not(any(
        all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
        all(
            feature = "std-icu",
            not(all(target_arch = "wasm32", target_os = "unknown"))
        )
    )),
    feature = "web-intl",
    all(target_arch = "wasm32", target_os = "unknown")
))]
pub type DefaultBackend = Intl;

/// The backend the statics format with (see the ICU4X build).
#[cfg(all(
    not(any(
        all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
        all(
            feature = "std-icu",
            not(all(target_arch = "wasm32", target_os = "unknown"))
        )
    )),
    not(all(
        feature = "web-intl",
        all(target_arch = "wasm32", target_os = "unknown")
    ))
))]
pub type DefaultBackend = Neutral;

#[cfg(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
))]
const DEFAULT_BACKEND: DefaultBackend = icu::Icu::NEW;

#[cfg(all(
    not(any(
        all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
        all(
            feature = "std-icu",
            not(all(target_arch = "wasm32", target_os = "unknown"))
        )
    )),
    feature = "web-intl",
    all(target_arch = "wasm32", target_os = "unknown")
))]
const DEFAULT_BACKEND: DefaultBackend = Intl;

#[cfg(all(
    not(any(
        all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
        all(
            feature = "std-icu",
            not(all(target_arch = "wasm32", target_os = "unknown"))
        )
    )),
    not(all(
        feature = "web-intl",
        all(target_arch = "wasm32", target_os = "unknown")
    ))
))]
const DEFAULT_BACKEND: DefaultBackend = Neutral;

// The strongest-wins rule, checked wherever such a build compiles, written
// from the features alone. In the browser: both formatters, ICU4X formats;
// `Intl` alone formats; none, the stub, whatever `std-icu` says.
#[cfg(all(
    feature = "web-icu",
    feature = "web-intl",
    all(target_arch = "wasm32", target_os = "unknown")
))]
const _: fn(DefaultBackend) -> icu::Icu = |backend| backend;
#[cfg(all(
    not(feature = "web-icu"),
    feature = "web-intl",
    all(target_arch = "wasm32", target_os = "unknown")
))]
const _: fn(DefaultBackend) -> Intl = |backend| backend;
#[cfg(all(
    not(feature = "web-icu"),
    not(feature = "web-intl"),
    all(target_arch = "wasm32", target_os = "unknown")
))]
const _: fn(DefaultBackend) -> Neutral = |backend| backend;
// Natively: `std-icu` formats with ICU4X whatever the browser's features
// say, and without it the stub formats, even with both of theirs on.
#[cfg(all(
    feature = "std-icu",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
const _: fn(DefaultBackend) -> icu::Icu = |backend| backend;
#[cfg(all(
    not(feature = "std-icu"),
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
const _: fn(DefaultBackend) -> Neutral = |backend| backend;

/// `:datetime`.
pub static DATETIME: DateTimeFunction = DateTimeFunction::datetime(DEFAULT_BACKEND);

/// `:date`.
pub static DATE: DateTimeFunction = DateTimeFunction::date(DEFAULT_BACKEND);

/// `:time`.
pub static TIME: DateTimeFunction = DateTimeFunction::time(DEFAULT_BACKEND);

/// Unannotated date/time values, for `Registry::with_dates`: as
/// `:datetime` with its defaults.
pub static DATES: DateTimeFunction = DateTimeFunction::unannotated(DEFAULT_BACKEND);

/// The date handlers of a generated module (`mf2-build`, `plan/08` §5.1),
/// for the ICU4X form the build chose for its corpus: `gregorian` or `any`
/// calendar, `zones` or `no_zones`. In a build that formats with ICU4X they
/// are `DATETIME`, `DATE`, `TIME` and `DATES` over `icu::Icu` of that form,
/// so only that variant is linked, and it reads a slice cut for it alone.
/// The form names types only: what it leaves out (a calendar, a zone style)
/// is an *Unsupported Operation* at run time, reported with a fallback.
/// Reached as `mf2::__date_statics!`; never written by hand.
#[cfg(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __date_statics {
    (gregorian no_zones) => {
        $crate::__date_statics!(@ $crate::icu::GregorianOnly, $crate::icu::NoZones);
    };
    (gregorian zones) => {
        $crate::__date_statics!(@ $crate::icu::GregorianOnly, $crate::icu::WithZones);
    };
    (any no_zones) => {
        $crate::__date_statics!(@ $crate::icu::AnyCalendar, $crate::icu::NoZones);
    };
    (any zones) => {
        $crate::__date_statics!(@ $crate::icu::AnyCalendar, $crate::icu::WithZones);
    };
    (@ $cal:ty, $zones:ty) => {
        /// `:datetime`.
        pub static DATETIME: $crate::DateTimeFunction<$crate::icu::Icu<$cal, $zones>> =
            $crate::DateTimeFunction::datetime($crate::icu::Icu::NEW);
        /// `:date`.
        pub static DATE: $crate::DateTimeFunction<$crate::icu::Icu<$cal, $zones>> =
            $crate::DateTimeFunction::date($crate::icu::Icu::NEW);
        /// `:time`.
        pub static TIME: $crate::DateTimeFunction<$crate::icu::Icu<$cal, $zones>> =
            $crate::DateTimeFunction::time($crate::icu::Icu::NEW);
        /// Unannotated date/time values.
        pub static DATES: $crate::DateTimeFunction<$crate::icu::Icu<$cal, $zones>> =
            $crate::DateTimeFunction::unannotated($crate::icu::Icu::NEW);
    };
}

/// The date handlers of a generated module, in a build that does not format
/// with ICU4X: the form is ICU4X's alone, so these are the crate's own
/// statics over its formatter.
#[cfg(not(any(
    all(feature = "web-icu", target_arch = "wasm32", target_os = "unknown"),
    all(
        feature = "std-icu",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    )
)))]
#[doc(hidden)]
#[macro_export]
macro_rules! __date_statics {
    ($calendars:ident $zones:ident) => {
        pub use $crate::{DATE, DATES, DATETIME, TIME};
    };
}
