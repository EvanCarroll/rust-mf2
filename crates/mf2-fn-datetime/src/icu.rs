//! The ICU4X backend (`std-icu` natively, `web-icu` in the browser):
//! a [`Plan`] becomes an
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
//! | data `D` | [`DefaultData`] (default) · [`Blob`] · `CachedBlob` · [`Compiled`] | `CachedBlob` with this side's cache feature (`std-cache` off the browser, `web-cache` in it), else [`Blob`] · the catalog's `icu.blob` LOCALE entry (client and server alike, so the same bytes), its provider and formatter built for each placeholder · the same, with the provider kept per catalog and the formatter per language and shape (per thread, so it needs `std`) · ICU4X's compiled data (off the browser, with this crate's `compiled-data` feature, for a registry written by hand: no feature of `mf2` turns it on) |
//!
//! [`Blob`] and `CachedBlob` never fall back to compiled data: a catalog without its blob
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

use alloc::boxed::Box;
use core::marker::PhantomData;

// In the browser ICU4X comes through `mf2-fn-datetime-web-icu` (`web-icu`),
// natively it is this crate's own (`std-icu`): see the manifest.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use mf2_fn_datetime_web_icu::{
    icu_calendar, icu_datetime, icu_locale_core, icu_provider, icu_provider_blob, icu_time,
    writeable,
};

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

// The cache is one side's: `std-cache` off the browser, `web-cache` in it.
// The other side's feature changes nothing here.
#[cfg(any(
    all(
        feature = "std-cache",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-cache", target_arch = "wasm32", target_os = "unknown")
))]
mod cache;
#[cfg(any(
    all(
        feature = "std-cache",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-cache", target_arch = "wasm32", target_os = "unknown")
))]
pub use cache::CachedBlob;

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

/// Data: the catalog's `icu.blob` LOCALE entry, its provider and formatter
/// built for each placeholder (the default without this side's cache).
#[derive(Clone, Copy, Debug)]
pub struct Blob;

/// The data [`Icu`] reads by default: [`CachedBlob`] with this side's cache
/// feature (`std-cache` off the browser, which `mf2`'s native ICU4X
/// formatter turns on; `web-cache` in it), else [`Blob`].
#[cfg(any(
    all(
        feature = "std-cache",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-cache", target_arch = "wasm32", target_os = "unknown")
))]
pub type DefaultData = CachedBlob;

/// The data [`Icu`] reads by default: `CachedBlob` with this side's cache
/// feature (`std-cache` off the browser, which `mf2`'s native ICU4X
/// formatter turns on; `web-cache` in it), else [`Blob`].
#[cfg(not(any(
    all(
        feature = "std-cache",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-cache", target_arch = "wasm32", target_os = "unknown")
)))]
pub type DefaultData = Blob;

/// Data: ICU4X's compiled data (never in the browser; `compiled-data`).
#[cfg(all(
    feature = "compiled-data",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[derive(Clone, Copy, Debug)]
pub struct Compiled;

/// The data an ICU4X formatter is built from.
#[doc(hidden)]
pub enum Source<'p> {
    /// A buffer provider: the catalog's blob, or a build-side recorder.
    Buffer(&'p dyn BufferProvider),
    /// ICU4X's compiled data.
    #[cfg(all(
        feature = "compiled-data",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
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

    /// Formats `plan` with variant `V` into `out` (`None`: checks only that
    /// it formats). Builds the formatter from this data each time, unless
    /// the data keeps it (`CachedBlob`).
    fn run<V: Variant>(
        cx: &FnContext<'_>,
        plan: &Plan<'_>,
        out: Option<&mut dyn Sink>,
    ) -> Result<(), FormatError> {
        Self::with(cx, |src| {
            V::run(&src, cx.locale(), plan.options, Some((plan, out)))
        })
    }
}

/// The catalog's `icu.blob` LOCALE entry; without it, an Unsupported
/// Operation.
fn blob<'x>(cx: &FnContext<'x>) -> Result<&'x [u8], FormatError> {
    cx.catalog()
        .locale_entry(mf2_catalog::format::locale_key::ICU_BLOB)
        .ok_or(FormatError::UnsupportedOperation)
}

/// The provider over a copy of `bytes`: it owns its blob (the catalog's
/// bytes are not `'static`).
fn provider(bytes: &[u8]) -> Result<icu_provider_blob::BlobDataProvider, FormatError> {
    icu_provider_blob::BlobDataProvider::try_new_from_blob(Box::from(bytes))
        .map_err(|_| FormatError::UnsupportedOperation)
}

impl Data for Blob {
    fn with<R>(
        cx: &FnContext<'_>,
        f: impl FnOnce(Source<'_>) -> Result<R, FormatError>,
    ) -> Result<R, FormatError> {
        let provider = provider(blob(cx)?)?;
        f(Source::Buffer(&provider))
    }
}

#[cfg(all(
    feature = "compiled-data",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
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
pub struct Icu<C = AnyCalendar, Z = WithZones, D = DefaultData>(Params<C, Z, D>);

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

    /// The formatter this variant builds (it owns its data: it outlives
    /// the provider it was built from).
    type Formatter: 'static;

    /// Builds the formatter of `locale` and the shape of `o`.
    fn build(
        src: &Source<'_>,
        locale: &str,
        o: &DateTimeOptions<'_>,
    ) -> Result<Self::Formatter, FormatError>;

    /// Formats `plan` with `f` into `out` (`None`: checks only). `zones` is
    /// the IANA parser when the plan has a zone style, else `None`.
    fn write(
        f: &Self::Formatter,
        plan: &Plan<'_>,
        zones: Option<&IanaParser>,
        out: Option<&mut dyn Sink>,
    ) -> Result<(), FormatError>;

    /// Builds (and with `run = Some`, formats): the formatter, and with a
    /// zone style the IANA parser.
    fn run(
        src: &Source<'_>,
        locale: &str,
        o: &DateTimeOptions<'_>,
        run: Option<(&Plan<'_>, Option<&mut dyn Sink>)>,
    ) -> Result<(), FormatError> {
        let f = Self::build(src, locale, o)?;
        let zones = match o.time_zone_style {
            Some(_) => Some(iana(src)?),
            None => None,
        };
        match run {
            Some((plan, out)) => Self::write(&f, plan, zones.as_ref(), out),
            // Building only (`prime`).
            None => Ok(()),
        }
    }
}

impl<C: 'static, Z: 'static, D: Data> Backend for Icu<C, Z, D>
where
    (C, Z): Variant,
{
    fn supports(&self, cx: &FnContext<'_>, plan: &Plan<'_>) -> Result<(), FormatError> {
        D::run::<(C, Z)>(cx, plan, None)
    }

    fn format(&self, cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
        let _ = D::run::<(C, Z)>(cx, plan, Some(out));
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
        Source::Buffer(p) => IanaParser::try_new_with_buffer_provider(*p)
            .map_err(|_| FormatError::UnsupportedOperation),
        #[cfg(all(
            feature = "compiled-data",
            not(all(target_arch = "wasm32", target_os = "unknown"))
        ))]
        Source::Compiled(_) => Ok(IanaParser::new().static_to_owned()),
    }
}

/// Sets the zone of `plan` on `input` (for a field set with a zone style).
fn zone(
    parser: &IanaParser,
    plan: &Plan<'_>,
    input: &mut DateTimeInputUnchecked,
) -> Result<(), FormatError> {
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
fn emit(formatted: &impl TryWriteable, out: Option<&mut dyn Sink>) {
    if let Some(out) = out {
        let _ = formatted.try_write_to(&mut SinkWrite(out));
    }
}

/// One variant's `build` and `write`: `$fmt` the formatter type, `$build`
/// the builder method of its field set, `$date` the input date.
macro_rules! variant {
    ($cal:ty, $zones:ty, $has_zones:expr, $gregorian:expr, $fmt:ty, $build:ident, $date:expr) => {
        impl Variant for ($cal, $zones) {
            const ZONES: bool = $has_zones;
            const GREGORIAN_ONLY: bool = $gregorian;

            type Formatter = $fmt;

            fn build(
                src: &Source<'_>,
                locale: &str,
                o: &DateTimeOptions<'_>,
            ) -> Result<$fmt, FormatError> {
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
                match src {
                    Source::Buffer(p) => <$fmt>::try_new_with_buffer_provider(*p, prefs, fs),
                    #[cfg(all(
                        feature = "compiled-data",
                        not(all(target_arch = "wasm32", target_os = "unknown"))
                    ))]
                    Source::Compiled(_) => <$fmt>::try_new(prefs, fs),
                }
                .map_err(|_| FormatError::UnsupportedOperation)
            }

            fn write(
                f: &$fmt,
                plan: &Plan<'_>,
                zones: Option<&IanaParser>,
                out: Option<&mut dyn Sink>,
            ) -> Result<(), FormatError> {
                let mut input = DateTimeInputUnchecked::default();
                let date = $date(f, plan)?;
                input.set_date_fields_unchecked(date);
                input.set_time_fields(time(plan)?);
                if let Some(parser) = zones {
                    zone(parser, plan, &mut input)?;
                }
                emit(&f.format_unchecked(input), out);
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
