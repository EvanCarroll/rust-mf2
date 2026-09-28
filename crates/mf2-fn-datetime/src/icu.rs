//! The ICU4X backend (`datetime-icu`, and the server side of
//! `datetime-intl`; `plans/03-runtime.md` §5.1–§5.2): a [`Plan`] becomes an
//! ICU4X semantic skeleton built at runtime (`FieldSetBuilder`), formatted
//! by `icu_datetime` with the plan's wall time and zone.
//!
//! What a registry links is chosen by **type**, so closed-world linking
//! prunes what a corpus does not use (B13; `mf2-build` picks the type):
//!
//! | Parameter | Values | Linked |
//! |---|---|---|
//! | calendar `C` | [`AnyCalendar`] (default) · [`GregorianOnly`] | `DateTimeFormatter` (every calendar; a locale's default, e.g. `th`'s Buddhist, and `calendar=`) · `FixedCalendarDateTimeFormatter<Gregorian>` (B4: ≤ 95 KB gz against ≤ 105) |
//! | zones `Z` | [`WithZones`] (default) · [`NoZones`] | the composite field set with time-zone styles · the date/time-only one — `timeZoneStyle` is then an *Unsupported Operation* (03 §5.2(1): −29 KB gz of code, ≈ −85 % of `icu.blob`) |
//! | data `D` | [`Blob`] (default) · [`Compiled`] | the catalog's `icu.blob` LOCALE entry (client and server alike, so the same bytes) · ICU4X's compiled data (off the browser: the server side of `datetime-intl`) |
//!
//! [`Blob`] never falls back to compiled data: a catalog without its blob
//! is an *Unsupported Operation*. The blob is built by `mf2-locale-data`'s
//! `icu-blob` feature by recording what [`prime`] constructs — the same code
//! as formatting — so it holds exactly the data this backend requests.
//!
//! The option mapping: `dateFields` / `fields` → `E`, `DE`, `MD`, `MDE`,
//! `YMD`, `YMDE`; `dateLength` / `length` → the field set's length (a
//! `:time` has none: medium); `timePrecision` / `precision` → `Hour`,
//! `Minute`, `Second`; `timeZoneStyle` → `SpecificLong` / `SpecificShort`
//! (what `Intl.DateTimeFormat`'s `timeZoneName` long / short show);
//! `hour12` → hour cycle `h12` / `h23`; `calendar` → the calendar algorithm
//! (with [`GregorianOnly`] anything but `gregory` is an *Unsupported
//! Operation*). The zone: UTC is ICU4X's `utc`, an offset an unknown zone
//! with that offset (localized-offset text, "GMT+05:30"), a named zone its
//! BCP-47 id through ICU4X's IANA parser, with the plan's offset.
//!
//! ICU4X writes through `core::fmt::Write` and carries its own `core::fmt`
//! and panic paths: B12 covers Rust MF2's crates, and this code is the
//! feature's documented cost (06 B4).

#[cfg(feature = "datetime-icu")]
use alloc::boxed::Box;
use core::marker::PhantomData;

use icu_calendar::Date as IcuDate;
use icu_datetime::fieldsets::builder::{DateFields as F, FieldSetBuilder, ZoneStyle as Zs};
use icu_datetime::fieldsets::enums::{CompositeDateTimeFieldSet, CompositeFieldSet};
use icu_datetime::options::{Length, TimePrecision as P};
use icu_datetime::unchecked::DateTimeInputUnchecked;
use icu_datetime::{
    DateTimeFormatter, DateTimeFormatterPreferences, FixedCalendarDateTimeFormatter,
};
use icu_locale_core::Locale;
use icu_locale_core::extensions::unicode::Value;
use icu_locale_core::preferences::extensions::unicode::keywords::{CalendarAlgorithm, HourCycle};
#[cfg(feature = "datetime-icu")]
use icu_provider::buf::BufferProvider;
use icu_time::zone::iana::IanaParser;
use icu_time::zone::{UtcOffset, ZoneNameTimestamp};
use icu_time::{Time, TimeZone};
use mf2_runtime::{
    DateFields, DateLength, DateStyle, DateTimeOptions, Dir, FnContext, FormatError, Sink,
    TimePrecision, ZoneOption, ZoneStyle,
};
use writeable::TryWriteable;

use crate::plan::{Backend, Plan};

/// Calendar support: every calendar (the default).
#[derive(Clone, Copy, Debug)]
pub struct AnyCalendar;

/// Calendar support: the Gregorian calendar only.
#[derive(Clone, Copy, Debug)]
pub struct GregorianOnly;

/// Zone support: `timeZoneStyle` (the default).
#[derive(Clone, Copy, Debug)]
pub struct WithZones;

/// Zone support: none — `timeZoneStyle` is an *Unsupported Operation*.
#[derive(Clone, Copy, Debug)]
pub struct NoZones;

/// Data: the catalog's `icu.blob` LOCALE entry (the default).
#[cfg(feature = "datetime-icu")]
#[derive(Clone, Copy, Debug)]
pub struct Blob;

/// Data: ICU4X's compiled data (never in the browser).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[derive(Clone, Copy, Debug)]
pub struct Compiled;

/// The data an ICU4X formatter is built from.
#[doc(hidden)]
pub enum Source<'p> {
    /// A buffer provider: the catalog's blob, or a build-side recorder.
    #[cfg(feature = "datetime-icu")]
    Buffer(&'p dyn BufferProvider),
    /// ICU4X's compiled data.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    Compiled(PhantomData<&'p ()>),
}

/// Where a backend's data comes from (see the module docs).
#[doc(hidden)]
pub trait Data: Sync + 'static {
    /// Runs `f` over this data for the catalog of `cx`.
    fn with<R>(
        cx: &FnContext<'_>,
        f: impl FnOnce(Source<'_>) -> Result<R, FormatError>,
    ) -> Result<R, FormatError>;
}

#[cfg(feature = "datetime-icu")]
impl Data for Blob {
    fn with<R>(
        cx: &FnContext<'_>,
        f: impl FnOnce(Source<'_>) -> Result<R, FormatError>,
    ) -> Result<R, FormatError> {
        let bytes = cx
            .catalog()
            .locale_entry(mf2_catalog::format::locale_key::ICU_BLOB)
            .ok_or(FormatError::UnsupportedOperation)?;
        // A copy: the provider owns its blob (the catalog's bytes are not
        // `'static`).
        let provider = icu_provider_blob::BlobDataProvider::try_new_from_blob(Box::from(bytes))
            .map_err(|_| FormatError::UnsupportedOperation)?;
        f(Source::Buffer(&provider))
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl Data for Compiled {
    fn with<R>(
        _cx: &FnContext<'_>,
        f: impl FnOnce(Source<'_>) -> Result<R, FormatError>,
    ) -> Result<R, FormatError> {
        f(Source::Compiled(PhantomData))
    }
}

/// The type parameters of [`Icu`] (it holds no value of them; `fn() -> _`
/// keeps it `Send`, `Sync` and `Copy` whatever they are).
type Params<C, Z, D> = PhantomData<fn() -> (C, Z, D)>;

/// The ICU4X backend over calendar support `C`, zone support `Z` and data
/// `D` (see the module docs). `Icu::NEW` builds one.
#[cfg(feature = "datetime-icu")]
pub struct Icu<C = AnyCalendar, Z = WithZones, D = Blob>(Params<C, Z, D>);

/// The ICU4X backend over calendar support `C`, zone support `Z` and data
/// `D` (see the module docs). `Icu::NEW` builds one.
#[cfg(not(feature = "datetime-icu"))]
pub struct Icu<C = AnyCalendar, Z = WithZones, D = Compiled>(Params<C, Z, D>);

impl<C, Z, D> Icu<C, Z, D> {
    /// The backend (it has no state).
    pub const NEW: Self = Icu(PhantomData);
}

impl<C, Z, D> Clone for Icu<C, Z, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, Z, D> Copy for Icu<C, Z, D> {}

impl<C, Z, D> core::fmt::Debug for Icu<C, Z, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Icu")
    }
}

/// A calendar × zones variant: builds the formatter for `o` from `src`
/// and, given a plan and a sink, formats. Implemented for the four pairs.
#[doc(hidden)]
pub trait Variant {
    /// Whether this variant shows time-zone styles.
    const ZONES: bool;

    /// Whether this variant formats the Gregorian calendar only.
    const GREGORIAN_ONLY: bool;

    /// Builds (and with `run = Some`, formats).
    fn run(
        src: &Source<'_>,
        locale: &str,
        o: &DateTimeOptions<'_>,
        run: Option<(&Plan<'_>, Option<&mut dyn Sink>)>,
    ) -> Result<(), FormatError>;
}

impl<C: 'static, Z: 'static, D: Data> Backend for Icu<C, Z, D>
where
    (C, Z): Variant,
{
    fn supports(&self, cx: &FnContext<'_>, plan: &Plan<'_>) -> Result<(), FormatError> {
        D::with(cx, |src| {
            <(C, Z)>::run(&src, cx.locale(), plan.options, Some((plan, None)))
        })
    }

    fn format(&self, cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
        let _ = D::with(cx, |src| {
            <(C, Z)>::run(&src, cx.locale(), plan.options, Some((plan, Some(out))))
        });
    }

    /// Localized text has the catalog's direction (Arabic dates are not
    /// left-to-right).
    fn dir(&self, cx: &FnContext<'_>, _plan: &Plan<'_>) -> Dir {
        cx.catalog().dir()
    }
}

/// `Sink` as the `fmt::Write` ICU4X writes to.
struct SinkWrite<'s>(&'s mut dyn Sink);

impl core::fmt::Write for SinkWrite<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.push_str(s);
        Ok(())
    }
}

/// The preferences: the locale, `hour12`, `calendar`.
fn prefs(
    locale: &str,
    o: &DateTimeOptions<'_>,
    gregorian_only: bool,
) -> Result<DateTimeFormatterPreferences, FormatError> {
    let loc = Locale::try_from_str(locale).map_err(|_| FormatError::UnsupportedOperation)?;
    let mut p = DateTimeFormatterPreferences::from(&loc);
    if let Some(h12) = o.hour12 {
        p.hour_cycle = Some(if h12 { HourCycle::H12 } else { HourCycle::H23 });
    }
    if let Some(cal) = o.calendar {
        if gregorian_only && cal != "gregory" {
            return Err(FormatError::UnsupportedOperation);
        }
        let v = Value::try_from_str(cal).map_err(|_| FormatError::UnsupportedOperation)?;
        let alg = CalendarAlgorithm::try_from(&v).map_err(|_| FormatError::UnsupportedOperation)?;
        p.calendar_algorithm = Some(alg);
    }
    Ok(p)
}

/// The field set builder for `o`. The option enums are not exhaustive: a
/// value this backend does not know is an Unsupported Operation, never a
/// silent default.
fn builder(o: &DateTimeOptions<'_>) -> Result<FieldSetBuilder, FormatError> {
    let mut b = FieldSetBuilder::new();
    if let Some(DateStyle { fields, length }) = o.date {
        b.date_fields = Some(match fields {
            DateFields::Weekday => F::E,
            DateFields::DayWeekday => F::DE,
            DateFields::MonthDay => F::MD,
            DateFields::MonthDayWeekday => F::MDE,
            DateFields::YearMonthDay => F::YMD,
            DateFields::YearMonthDayWeekday => F::YMDE,
            _ => return Err(FormatError::UnsupportedOperation),
        });
        b.length = Some(match length {
            DateLength::Long => Length::Long,
            DateLength::Medium => Length::Medium,
            DateLength::Short => Length::Short,
            _ => return Err(FormatError::UnsupportedOperation),
        });
    }
    b.time_precision = match o.time {
        None => None,
        Some(TimePrecision::Hour) => Some(P::Hour),
        Some(TimePrecision::Minute) => Some(P::Minute),
        Some(TimePrecision::Second) => Some(P::Second),
        Some(_) => return Err(FormatError::UnsupportedOperation),
    };
    Ok(b)
}

/// The ICU4X time of `plan`.
fn time(plan: &Plan<'_>) -> Result<Time, FormatError> {
    let t = plan.time;
    Time::try_new(
        t.hour(),
        t.minute(),
        t.second(),
        u32::from(t.millisecond()) * 1_000_000,
    )
    .map_err(|_| FormatError::UnsupportedOperation)
}

/// The IANA parser over `src`.
fn iana(src: &Source<'_>) -> Result<IanaParser, FormatError> {
    match src {
        #[cfg(feature = "datetime-icu")]
        Source::Buffer(p) => IanaParser::try_new_with_buffer_provider(*p)
            .map_err(|_| FormatError::UnsupportedOperation),
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        Source::Compiled(_) => Ok(IanaParser::new().static_to_owned()),
    }
}

/// Sets the zone of `plan` on `input` (for a field set with a zone style).
fn zone(
    src: &Source<'_>,
    plan: &Plan<'_>,
    input: &mut DateTimeInputUnchecked,
) -> Result<(), FormatError> {
    let parser = iana(src)?;
    let (id, offset) = match plan.zone {
        ZoneOption::Offset(o) => (TimeZone::UNKNOWN, Some(o)),
        ZoneOption::Named(n) => (parser.as_borrowed().parse(n), plan.offset),
        ZoneOption::Utc | ZoneOption::Input => (parser.as_borrowed().parse("Etc/UTC"), Some(0)),
        _ => return Err(FormatError::UnsupportedOperation),
    };
    input.set_time_zone_id(id);
    if let Some(o) = offset {
        let o = UtcOffset::try_from_seconds(o).map_err(|_| FormatError::UnsupportedOperation)?;
        input.set_time_zone_utc_offset(o);
    }
    let t = plan.epoch_ms().unwrap_or_else(|| plan.wall_ms());
    input.set_time_zone_name_timestamp(ZoneNameTimestamp::from_epoch_seconds(t.div_euclid(1000)));
    Ok(())
}

/// Writes `formatted` to `out` (a lossy fallback, e.g. a zone name the data
/// lacks, is written as ICU4X gives it).
fn write(formatted: &impl TryWriteable, out: Option<&mut dyn Sink>) {
    if let Some(out) = out {
        let _ = formatted.try_write_to(&mut SinkWrite(out));
    }
}

/// One variant's `run`: `$fset` the field set type, `$build` its builder
/// method, `$new` the formatter constructor path, `$date` the input date.
macro_rules! variant {
    ($cal:ty, $zones:ty, $has_zones:expr, $gregorian:expr, $fmt:ty, $build:ident, $date:expr) => {
        impl Variant for ($cal, $zones) {
            const ZONES: bool = $has_zones;
            const GREGORIAN_ONLY: bool = $gregorian;

            fn run(
                src: &Source<'_>,
                locale: &str,
                o: &DateTimeOptions<'_>,
                run: Option<(&Plan<'_>, Option<&mut dyn Sink>)>,
            ) -> Result<(), FormatError> {
                let mut b = builder(o)?;
                if let Some(style) = o.time_zone_style {
                    if !$has_zones {
                        return Err(FormatError::UnsupportedOperation);
                    }
                    b.zone_style = Some(match style {
                        ZoneStyle::Long => Zs::SpecificLong,
                        ZoneStyle::Short => Zs::SpecificShort,
                        _ => return Err(FormatError::UnsupportedOperation),
                    });
                }
                let fs = b.$build().map_err(|_| FormatError::UnsupportedOperation)?;
                let prefs = prefs(locale, o, $gregorian)?;
                let f = match src {
                    #[cfg(feature = "datetime-icu")]
                    Source::Buffer(p) => <$fmt>::try_new_with_buffer_provider(*p, prefs, fs),
                    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
                    Source::Compiled(_) => <$fmt>::try_new(prefs, fs),
                }
                .map_err(|_| FormatError::UnsupportedOperation)?;
                let mut input = DateTimeInputUnchecked::default();
                let Some((plan, out)) = run else {
                    // Building only (`prime`): with a zone style, the IANA
                    // parser too.
                    if o.time_zone_style.is_some() {
                        iana(src)?;
                    }
                    return Ok(());
                };
                let date = $date(&f, plan)?;
                input.set_date_fields_unchecked(date);
                input.set_time_fields(time(plan)?);
                if o.time_zone_style.is_some() {
                    zone(src, plan, &mut input)?;
                }
                write(&f.format_unchecked(input), out);
                Ok(())
            }
        }
    };
}

/// The Gregorian date of `plan`.
fn gregorian<F>(
    _f: &F,
    plan: &Plan<'_>,
) -> Result<IcuDate<icu_calendar::cal::Gregorian>, FormatError> {
    let d = plan.date;
    IcuDate::try_new_gregorian(d.year(), d.month(), d.day())
        .map_err(|_| FormatError::UnsupportedOperation)
}

/// The date of `plan` in the formatter's calendar.
fn in_calendar<'f, FSet: icu_datetime::scaffold::DateTimeMarkers>(
    f: &'f DateTimeFormatter<FSet>,
    plan: &Plan<'_>,
) -> Result<IcuDate<icu_calendar::Ref<'f, icu_calendar::AnyCalendar>>, FormatError> {
    let d = plan.date;
    let iso = IcuDate::try_new_iso(d.year(), d.month(), d.day())
        .map_err(|_| FormatError::UnsupportedOperation)?;
    Ok(iso.to_calendar(f.calendar()))
}

variant!(
    AnyCalendar,
    WithZones,
    true,
    false,
    DateTimeFormatter<CompositeFieldSet>,
    build_composite,
    in_calendar
);
variant!(
    AnyCalendar,
    NoZones,
    false,
    false,
    DateTimeFormatter<CompositeDateTimeFieldSet>,
    build_composite_datetime,
    in_calendar
);
variant!(
    GregorianOnly,
    WithZones,
    true,
    true,
    FixedCalendarDateTimeFormatter<icu_calendar::cal::Gregorian, CompositeFieldSet>,
    build_composite,
    gregorian
);
variant!(
    GregorianOnly,
    NoZones,
    false,
    true,
    FixedCalendarDateTimeFormatter<icu_calendar::cal::Gregorian, CompositeDateTimeFieldSet>,
    build_composite_datetime,
    gregorian
);

#[cfg(feature = "datetime-icu")]
/// Build side (`mf2-locale-data`'s `icu-blob`): constructs, through
/// `provider`, the formatter `Icu<C, Z, _>` builds for `locale` under each
/// of `shapes` — the options a plan can carry: the date and time parts, the
/// zone style, `hour12`, `calendar` — so that a recording `provider` sees
/// every data request formatting them can make (formatting requests nothing
/// that building does not, but the IANA zone parser, which a shape with a
/// zone style builds too). A shape this variant cannot format is skipped: a
/// zone style without zones, a calendar other than `gregory` with
/// [`GregorianOnly`]. Returns the shapes that failed to build, with their
/// error.
#[doc(hidden)]
pub fn prime<'c, C: 'static, Z: 'static>(
    provider: &dyn BufferProvider,
    locale: &str,
    shapes: &[DateTimeOptions<'c>],
) -> alloc::vec::Vec<(DateTimeOptions<'c>, FormatError)>
where
    (C, Z): Variant,
{
    let src = Source::Buffer(provider);
    let mut failed = alloc::vec::Vec::new();
    for o in shapes {
        if (o.time_zone_style.is_some() && !<(C, Z)>::ZONES)
            || (<(C, Z)>::GREGORIAN_ONLY && o.calendar.is_some_and(|c| c != "gregory"))
        {
            continue;
        }
        if let Err(e) = <(C, Z)>::run(&src, locale, o, None) {
            failed.push((*o, e));
        }
    }
    failed
}
