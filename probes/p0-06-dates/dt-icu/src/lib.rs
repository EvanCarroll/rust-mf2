//! P0.6 probe — the `datetime-icu` backend: an [`mf2dt::Plan`] and a resolved
//! value become text through ICU4X `icu_datetime` (semantic skeletons, built at
//! runtime with `FieldSetBuilder`, since MF2 options arrive at runtime).
//!
//! Two formatter flavours × two data sources:
//! * `*_fixed_*` — `FixedCalendarDateTimeFormatter<Gregorian, _>` (the smallest
//!   ICU4X datetime formatter; ignores `calendar`, reports it unsupported);
//! * `*_any_*`   — `DateTimeFormatter<_>` (any calendar, honours `calendar`
//!   and the locale's default calendar);
//! * `buffer`    — data from a blob (`BufferProvider`), `compiled` — baked data.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use icu_calendar::Date;
use icu_datetime::fieldsets::builder::{DateFields as IDF, FieldSetBuilder, ZoneStyle as IZS};
use icu_datetime::fieldsets::enums::CompositeFieldSet;
use icu_datetime::options::{Length as IL, TimePrecision};
use icu_datetime::unchecked::DateTimeInputUnchecked;
use icu_datetime::DateTimeFormatterPreferences;
use icu_locale_core::Locale;
use icu_locale_core::extensions::unicode::Value;
use icu_locale_core::preferences::extensions::unicode::keywords::{CalendarAlgorithm, HourCycle};
use icu_time::zone::{UtcOffset, ZoneNameTimestamp};
use icu_time::{Time, TimeZone};
use mf2dt::{Civil, DateFields, Error, Length, Plan, Precision, ZoneStyle, Zoned};
use writeable::TryWriteable;

/// MF2 plan → ICU4X dynamic field set.
pub fn field_set(plan: &Plan<'_>) -> Option<CompositeFieldSet> {
    let mut b = FieldSetBuilder::new();
    b.length = Some(match plan.length {
        Length::Long => IL::Long,
        Length::Medium => IL::Medium,
        Length::Short => IL::Short,
    });
    b.date_fields = plan.date.map(|d| match d {
        DateFields::Weekday => IDF::E,
        DateFields::DayWeekday => IDF::DE,
        DateFields::MonthDay => IDF::MD,
        DateFields::MonthDayWeekday => IDF::MDE,
        DateFields::YearMonthDay => IDF::YMD,
        DateFields::YearMonthDayWeekday => IDF::YMDE,
    });
    b.time_precision = plan.time.map(|p| match p {
        Precision::Hour => TimePrecision::Hour,
        Precision::Minute => TimePrecision::Minute,
        Precision::Second => TimePrecision::Second,
    });
    b.zone_style = plan.zone.map(|z| match z {
        // Offset-only zones (all the ICU4X backend can resolve without a tz
        // database) render as localized offsets either way; this variant
        // links and loads only the offset formats.
        #[cfg(feature = "offset-zones")]
        ZoneStyle::Long => IZS::LocalizedOffsetLong,
        #[cfg(feature = "offset-zones")]
        ZoneStyle::Short => IZS::LocalizedOffsetShort,
        #[cfg(not(feature = "offset-zones"))]
        ZoneStyle::Long => IZS::SpecificLong,
        #[cfg(not(feature = "offset-zones"))]
        ZoneStyle::Short => IZS::SpecificShort,
    });
    b.build_composite().ok()
}

/// Same without zones: `CompositeDateTimeFieldSet` — for a corpus that never
/// uses `timeZoneStyle` (the build knows), time-zone name code is not linked.
pub fn field_set_no_zone(plan: &Plan<'_>) -> Option<icu_datetime::fieldsets::enums::CompositeDateTimeFieldSet> {
    if plan.zone.is_some() {
        return None;
    }
    let mut b = FieldSetBuilder::new();
    b.length = Some(match plan.length {
        Length::Long => IL::Long,
        Length::Medium => IL::Medium,
        Length::Short => IL::Short,
    });
    b.date_fields = plan.date.map(|d| match d {
        DateFields::Weekday => IDF::E,
        DateFields::DayWeekday => IDF::DE,
        DateFields::MonthDay => IDF::MD,
        DateFields::MonthDayWeekday => IDF::MDE,
        DateFields::YearMonthDay => IDF::YMD,
        DateFields::YearMonthDayWeekday => IDF::YMDE,
    });
    b.time_precision = plan.time.map(|p| match p {
        Precision::Hour => TimePrecision::Hour,
        Precision::Minute => TimePrecision::Minute,
        Precision::Second => TimePrecision::Second,
    });
    b.build_composite_datetime().ok()
}

/// Locale + override options → ICU4X preferences.
pub fn prefs(locale: &str, plan: &Plan<'_>) -> Result<DateTimeFormatterPreferences, Error> {
    let loc = Locale::try_from_str(locale).map_err(|_| Error::UnsupportedOperation)?;
    let mut p = DateTimeFormatterPreferences::from(&loc);
    if let Some(h12) = plan.ov.hour12 {
        p.hour_cycle = Some(if h12 { HourCycle::H12 } else { HourCycle::H23 });
    }
    if let Some(cal) = plan.ov.calendar {
        let v = Value::try_from_str(cal).map_err(|_| Error::BadOption)?;
        p.calendar_algorithm = Some(CalendarAlgorithm::try_from(&v).map_err(|_| Error::BadOption)?);
    }
    Ok(p)
}

/// Resolved value → ICU4X time + zone inputs (the date is set per calendar).
fn time_zone_input(z: &Zoned<'_>) -> Result<(DateTimeInputUnchecked, Civil), Error> {
    let mut input = DateTimeInputUnchecked::default();
    let civil = match *z {
        Zoned::Fixed { civil, offset } => {
            let off =
                UtcOffset::try_from_seconds(offset * 60).map_err(|_| Error::BadOption)?;
            input.set_time_zone_id(TimeZone::UNKNOWN);
            input.set_time_zone_utc_offset(off);
            // Name resolution needs the instant, even for an offset-only zone.
            let secs = civil.epoch_ms_as_utc().div_euclid(1000) - i64::from(offset) * 60;
            input.set_time_zone_name_timestamp(ZoneNameTimestamp::from_epoch_seconds(secs));
            civil
        }
        // ICU4X has no time-zone transition rules: converting to, or naming, an
        // IANA zone needs a tz database next to it (see RESULT.md).
        Zoned::Named { .. } => return Err(Error::UnsupportedOperation),
    };
    let t = Time::try_new(
        civil.hour,
        civil.minute,
        civil.second,
        u32::from(civil.millis) * 1_000_000,
    )
    .map_err(|_| Error::BadOperand)?;
    input.set_time_fields(t);
    Ok((input, civil))
}

fn write(
    out: &mut String,
    f: &impl TryWriteable,
) -> Result<(), Error> {
    match f.try_write_to(out) {
        Ok(Ok(())) => Ok(()),
        _ => Err(Error::UnsupportedOperation),
    }
}

#[cfg(feature = "buffer")]
pub use buffer::*;
#[cfg(feature = "buffer")]
mod buffer {
    use super::*;
    use icu_calendar::cal::Gregorian;
    use icu_datetime::{DateTimeFormatter, FixedCalendarDateTimeFormatter};
    use icu_provider::buf::{BufferMarker, BufferProvider};
    use icu_provider::prelude::*;

    /// A per-locale catalog blob holds exactly two data locales: the catalog's
    /// own (fully resolved at export, `DeduplicationStrategy::None`) and `und`
    /// (root-only, attribute-keyed entries the exporter never copies down).
    /// This adapter is the whole runtime "fallback": exact id → (catalog
    /// locale, same attributes) → (`und`, same attributes). It replaces
    /// ICU4X's `LocaleFallbackProvider`, which would need likely-subtags and
    /// parent data plus their code in the wasm.
    pub struct OneLocale<P> {
        pub inner: P,
        pub locale: DataLocale,
    }

    impl<P: BufferProvider> DynamicDataProvider<BufferMarker> for OneLocale<P> {
        fn load_data(
            &self,
            marker: DataMarkerInfo,
            req: DataRequest<'_>,
        ) -> Result<DataResponse<BufferMarker>, DataError> {
            let retry = |locale: &DataLocale| {
                let mut r = req;
                r.id = DataIdentifierBorrowed::for_marker_attributes_and_locale(
                    req.id.marker_attributes,
                    locale,
                );
                self.inner.load_data(marker, r)
            };
            match self.inner.load_data(marker, req) {
                Err(e) if e.kind == DataErrorKind::IdentifierNotFound => match retry(&self.locale) {
                    Err(e) if e.kind == DataErrorKind::IdentifierNotFound => {
                        retry(&DataLocale::default())
                    }
                    r => r,
                },
                r => r,
            }
        }
    }

    /// Gregorian-only formatter, data from a blob.
    pub fn format_fixed_buffer<P: BufferProvider + ?Sized>(
        provider: &P,
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        if plan.ov.calendar.is_some_and(|c| c != "gregory") {
            return Err(Error::UnsupportedOperation);
        }
        let fs = field_set(plan).ok_or(Error::BadOption)?;
        let f = FixedCalendarDateTimeFormatter::<Gregorian, _>::try_new_with_buffer_provider(
            provider,
            prefs(locale, plan)?,
            fs,
        )
        .map_err(|_| Error::UnsupportedOperation)?;
        let (mut input, c) = time_zone_input(z)?;
        let d = Date::try_new_gregorian(c.year, c.month, c.day).map_err(|_| Error::BadOperand)?;
        input.set_date_fields_unchecked(d);
        write(out, &f.format_unchecked(input))
    }

    /// Gregorian-only formatter without time-zone support, data from a blob.
    pub fn format_fixed_nozone_buffer<P: BufferProvider + ?Sized>(
        provider: &P,
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        if plan.ov.calendar.is_some_and(|c| c != "gregory") {
            return Err(Error::UnsupportedOperation);
        }
        let fs = field_set_no_zone(plan).ok_or(Error::UnsupportedOperation)?;
        let f = FixedCalendarDateTimeFormatter::<Gregorian, _>::try_new_with_buffer_provider(
            provider,
            prefs(locale, plan)?,
            fs,
        )
        .map_err(|_| Error::UnsupportedOperation)?;
        let (mut input, c) = time_zone_input(z)?;
        let d = Date::try_new_gregorian(c.year, c.month, c.day).map_err(|_| Error::BadOperand)?;
        input.set_date_fields_unchecked(d);
        write(out, &f.format_unchecked(input))
    }

    /// Any-calendar formatter, data from a blob.
    pub fn format_any_buffer<P: BufferProvider + ?Sized>(
        provider: &P,
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        let fs = field_set(plan).ok_or(Error::BadOption)?;
        let f = DateTimeFormatter::try_new_with_buffer_provider(provider, prefs(locale, plan)?, fs)
            .map_err(|_| Error::UnsupportedOperation)?;
        let (mut input, c) = time_zone_input(z)?;
        let d = Date::try_new_iso(c.year, c.month, c.day).map_err(|_| Error::BadOperand)?;
        input.set_date_fields_unchecked(d.to_calendar(f.calendar()));
        write(out, &f.format_unchecked(input))
    }
}

#[cfg(feature = "compiled")]
pub use compiled::*;
#[cfg(feature = "compiled")]
mod compiled {
    use super::*;
    use icu_calendar::cal::Gregorian;
    use icu_datetime::{DateTimeFormatter, FixedCalendarDateTimeFormatter};

    /// Gregorian-only formatter, compiled data.
    pub fn format_fixed_compiled(
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        if plan.ov.calendar.is_some_and(|c| c != "gregory") {
            return Err(Error::UnsupportedOperation);
        }
        let fs = field_set(plan).ok_or(Error::BadOption)?;
        let f = FixedCalendarDateTimeFormatter::<Gregorian, _>::try_new(prefs(locale, plan)?, fs)
            .map_err(|_| Error::UnsupportedOperation)?;
        let (mut input, c) = time_zone_input(z)?;
        let d = Date::try_new_gregorian(c.year, c.month, c.day).map_err(|_| Error::BadOperand)?;
        input.set_date_fields_unchecked(d);
        write(out, &f.format_unchecked(input))
    }

    /// Any-calendar formatter, compiled data.
    pub fn format_any_compiled(
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        let fs = field_set(plan).ok_or(Error::BadOption)?;
        let f = DateTimeFormatter::try_new(prefs(locale, plan)?, fs)
            .map_err(|_| Error::UnsupportedOperation)?;
        let (mut input, c) = time_zone_input(z)?;
        let d = Date::try_new_iso(c.year, c.month, c.day).map_err(|_| Error::BadOperand)?;
        input.set_date_fields_unchecked(d.to_calendar(f.calendar()));
        write(out, &f.format_unchecked(input))
    }
}
