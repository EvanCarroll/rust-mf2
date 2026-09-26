//! The `icu.blob` LOCALE entry (`plans/02-catalog-format.md` §4.9, key 48;
//! `plans/11` A6): per locale, an ICU4X postcard data blob holding exactly
//! what `mf2-fn-datetime`'s `datetime-icu` backend requests for what the
//! corpus formats.
//!
//! **How it is restricted.** The backend's own [`mf2_fn_datetime::icu::prime`]
//! — the same construction code that formats — builds the formatter of
//! every shape the corpus can format ([`DateNeeds`]: the date and time
//! parts, zone styles, `hour12` values and calendars its expressions and
//! literal options give, `plans/02-catalog-format.md` §4.4) through a
//! *recording* buffer provider: each request is answered from ICU4X's
//! compiled data (the `icu_*_data` crates' baked CLDR 48 tables — nothing is
//! downloaded, as in P0.6's `blobgen`) and remembered, and the blob is
//! exactly the requests seen, under the identifiers requested. So the
//! client finds every key it asks for (no runtime locale fallback, no `und`
//! copies), and nothing it never asks for is shipped — one numbering
//! system's digits, not all; no zone names unless a shape has a zone style.
//!
//! **Backend variants** ([`IcuBlobSpec`]): any calendar or Gregorian only,
//! with or without zone styles (`Icu<AnyCalendar | GregorianOnly, WithZones
//! | NoZones>`); a wider variant's blob also carries the narrower ones'
//! requests, so one blob serves each of them.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use icu_provider::buf::{BufferFormat, BufferMarker};
use icu_provider::dynutil::UpcastDataPayload;
use icu_provider::export::{DataExporter, ExportMarker, FlushMetadata};
use icu_provider::prelude::*;
use icu_provider_export::blob_exporter::BlobExporter;
use mf2_fn_datetime::icu::{AnyCalendar, GregorianOnly, NoZones, Variant, WithZones, prime};
use mf2_fn_datetime::literal_options;
use mf2_model::{Declaration, Expression, FunctionRef, Message, OptionValue, Pattern, PatternPart};
use mf2_runtime::{DateFields, DateLength, DateStyle, DateTimeOptions, TimePrecision, ZoneStyle};

use crate::error::Error;
use crate::number::Selection;

/// ICU4X's compiled data, as a provider of every marker the backend can
/// request.
struct Src;

// The baked-data macros name `alloc`.
extern crate alloc;

const _: () = {
    // Baked data refers to the locale fallbacker by this name.
    use icu_locale::fallback as icu_locale_fallback;
    icu_datetime_data::make_provider!(Src);
    icu_datetime_data::impl_datetime_names_dayperiod_v1!(Src);
    icu_datetime_data::impl_datetime_names_weekday_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_buddhist_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_chinese_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_coptic_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_dangi_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_ethiopian_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_gregorian_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_hebrew_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_hijri_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_indian_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_japanese_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_persian_v1!(Src);
    icu_datetime_data::impl_datetime_names_month_roc_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_buddhist_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_chinese_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_coptic_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_dangi_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_ethiopian_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_gregorian_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_hebrew_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_hijri_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_indian_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_japanese_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_persian_v1!(Src);
    icu_datetime_data::impl_datetime_names_year_roc_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_buddhist_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_chinese_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_coptic_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_dangi_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_ethiopian_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_gregorian_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_hebrew_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_hijri_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_indian_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_japanese_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_persian_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_date_roc_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_glue_v1!(Src);
    icu_datetime_data::impl_datetime_patterns_time_v1!(Src);
    icu_datetime_data::impl_timezone_names_essentials_v1!(Src);
    icu_datetime_data::impl_timezone_names_specific_long_v1!(Src);
    icu_datetime_data::impl_timezone_names_specific_short_v1!(Src);
    icu_datetime_data::impl_timezone_names_generic_long_v1!(Src);
    icu_datetime_data::impl_timezone_names_generic_short_v1!(Src);
    icu_datetime_data::impl_timezone_names_standard_long_v1!(Src);
    icu_datetime_data::impl_timezone_names_locations_root_v1!(Src);
    icu_datetime_data::impl_timezone_names_locations_override_v1!(Src);
    icu_datetime_data::impl_timezone_names_cities_root_v1!(Src);
    icu_datetime_data::impl_timezone_names_cities_override_v1!(Src);
    icu_time_data::impl_timezone_periods_v1!(Src);
    icu_time_data::impl_timezone_identifiers_iana_core_v1!(Src);
    icu_calendar_data::impl_calendar_japanese_modern_v1!(Src);
    icu_calendar_data::impl_calendar_preferred_v1!(Src);
    icu_calendar_data::impl_calendar_week_v1!(Src);
    icu_decimal_data::impl_decimal_symbols_v1!(Src);
    icu_decimal_data::impl_decimal_digits_v1!(Src);
};

/// `Src` as an export-marker provider: a dynamic dispatch over the marker
/// list.
macro_rules! exportable {
    ($($m:ty),+ $(,)?) => {
        impl DynamicDataProvider<ExportMarker> for Src {
            fn load_data(
                &self,
                marker: DataMarkerInfo,
                req: DataRequest<'_>,
            ) -> Result<DataResponse<ExportMarker>, DataError> {
                $(
                    if marker == <$m>::INFO {
                        let r = DataProvider::<$m>::load(self, req)?;
                        return Ok(DataResponse {
                            metadata: r.metadata,
                            payload: ExportMarker::upcast(r.payload),
                        });
                    }
                )+
                Err(DataErrorKind::MarkerNotFound.with_req(marker, req))
            }
        }
    };
}

exportable!(
    icu_datetime::provider::names::DatetimeNamesDayperiodV1,
    icu_datetime::provider::names::DatetimeNamesWeekdayV1,
    icu_datetime::provider::names::DatetimeNamesMonthBuddhistV1,
    icu_datetime::provider::names::DatetimeNamesMonthChineseV1,
    icu_datetime::provider::names::DatetimeNamesMonthCopticV1,
    icu_datetime::provider::names::DatetimeNamesMonthDangiV1,
    icu_datetime::provider::names::DatetimeNamesMonthEthiopianV1,
    icu_datetime::provider::names::DatetimeNamesMonthGregorianV1,
    icu_datetime::provider::names::DatetimeNamesMonthHebrewV1,
    icu_datetime::provider::names::DatetimeNamesMonthHijriV1,
    icu_datetime::provider::names::DatetimeNamesMonthIndianV1,
    icu_datetime::provider::names::DatetimeNamesMonthJapaneseV1,
    icu_datetime::provider::names::DatetimeNamesMonthPersianV1,
    icu_datetime::provider::names::DatetimeNamesMonthRocV1,
    icu_datetime::provider::names::DatetimeNamesYearBuddhistV1,
    icu_datetime::provider::names::DatetimeNamesYearChineseV1,
    icu_datetime::provider::names::DatetimeNamesYearCopticV1,
    icu_datetime::provider::names::DatetimeNamesYearDangiV1,
    icu_datetime::provider::names::DatetimeNamesYearEthiopianV1,
    icu_datetime::provider::names::DatetimeNamesYearGregorianV1,
    icu_datetime::provider::names::DatetimeNamesYearHebrewV1,
    icu_datetime::provider::names::DatetimeNamesYearHijriV1,
    icu_datetime::provider::names::DatetimeNamesYearIndianV1,
    icu_datetime::provider::names::DatetimeNamesYearJapaneseV1,
    icu_datetime::provider::names::DatetimeNamesYearPersianV1,
    icu_datetime::provider::names::DatetimeNamesYearRocV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateBuddhistV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateChineseV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateCopticV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateDangiV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateEthiopianV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateGregorianV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateHebrewV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateHijriV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateIndianV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateJapaneseV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDatePersianV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsDateRocV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsGlueV1,
    icu_datetime::provider::semantic_skeletons::DatetimePatternsTimeV1,
    icu_datetime::provider::time_zones::TimezoneNamesEssentialsV1,
    icu_datetime::provider::time_zones::TimezoneNamesSpecificLongV1,
    icu_datetime::provider::time_zones::TimezoneNamesSpecificShortV1,
    icu_datetime::provider::time_zones::TimezoneNamesGenericLongV1,
    icu_datetime::provider::time_zones::TimezoneNamesGenericShortV1,
    icu_datetime::provider::time_zones::TimezoneNamesStandardLongV1,
    icu_datetime::provider::time_zones::TimezoneNamesLocationsRootV1,
    icu_datetime::provider::time_zones::TimezoneNamesLocationsOverrideV1,
    icu_datetime::provider::time_zones::TimezoneNamesCitiesRootV1,
    icu_datetime::provider::time_zones::TimezoneNamesCitiesOverrideV1,
    icu_time::provider::TimezonePeriodsV1,
    icu_time::provider::iana::TimezoneIdentifiersIanaCoreV1,
    icu_calendar::provider::CalendarJapaneseModernV1,
    icu_calendar::provider::CalendarPreferredV1,
    icu_calendar::provider::CalendarWeekV1,
    icu_decimal::provider::DecimalSymbolsV1,
    icu_decimal::provider::DecimalDigitsV1,
);

/// One recorded response: its payload, the postcard bytes served, and its
/// checksum (ICU4X cross-checks the zone-id data by it).
struct Recorded {
    payload: DataPayload<ExportMarker>,
    bytes: Box<[u8]>,
    checksum: Option<u64>,
}

/// Serves `Src` as postcard buffers, remembering every request.
#[derive(Default)]
struct Recorder {
    seen: Mutex<BTreeMap<(DataMarkerInfo, DataIdentifierCow<'static>), Recorded>>,
}

fn postcard(payload: &DataPayload<ExportMarker>) -> Result<Vec<u8>, DataError> {
    use postcard::ser_flavors::Flavor;
    let mut ser = postcard::Serializer {
        output: postcard::ser_flavors::AllocVec::new(),
    };
    payload.serialize(&mut ser)?;
    ser.output
        .finalize()
        .map_err(|_| DataError::custom("postcard serialization"))
}

impl DynamicDataProvider<BufferMarker> for Recorder {
    fn load_data(
        &self,
        marker: DataMarkerInfo,
        req: DataRequest<'_>,
    ) -> Result<DataResponse<BufferMarker>, DataError> {
        let key = (marker, req.id.into_owned());
        let mut seen = self
            .seen
            .lock()
            .map_err(|_| DataError::custom("recorder lock"))?;
        let (bytes, checksum) = if let Some(r) = seen.get(&key) {
            (r.bytes.clone(), r.checksum)
        } else {
            let r = DynamicDataProvider::<ExportMarker>::load_data(&Src, marker, req)?;
            let bytes: Box<[u8]> = postcard(&r.payload)?.into_boxed_slice();
            let checksum = r.metadata.checksum;
            seen.insert(
                key,
                Recorded {
                    payload: r.payload,
                    bytes: bytes.clone(),
                    checksum,
                },
            );
            (bytes, checksum)
        };
        let mut metadata = DataResponseMetadata::default();
        metadata.buffer_format = Some(BufferFormat::Postcard1);
        metadata.checksum = checksum;
        Ok(DataResponse {
            metadata,
            payload: DataPayload::from_owned_buffer(bytes),
        })
    }
}

/// A `Write` into a shared buffer (the exporter wants to own its sink).
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("blob buffer lock"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A shape a corpus formats: the date part (fields and length), the time
/// part (precision), the zone style — what selects an ICU4X formatter's
/// patterns and names, besides `hour12` and the calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Shape {
    date: Option<(u8, u8)>,
    time: Option<u8>,
    zone: Option<u8>,
}

const FIELDS: [DateFields; 6] = [
    DateFields::Weekday,
    DateFields::DayWeekday,
    DateFields::MonthDay,
    DateFields::MonthDayWeekday,
    DateFields::YearMonthDay,
    DateFields::YearMonthDayWeekday,
];
const LENGTHS: [DateLength; 3] = [DateLength::Long, DateLength::Medium, DateLength::Short];
const PRECISIONS: [TimePrecision; 3] = [
    TimePrecision::Hour,
    TimePrecision::Minute,
    TimePrecision::Second,
];
const ZONE_STYLES: [ZoneStyle; 2] = [ZoneStyle::Long, ZoneStyle::Short];

/// The index of `v` in `table` (every value is in its table).
fn index<T: PartialEq>(table: &[T], v: &T) -> u8 {
    table
        .iter()
        .position(|t| t == v)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

impl Shape {
    /// The shape of `o` (its date and time parts and zone style).
    pub fn of(o: &DateTimeOptions<'_>) -> Shape {
        Shape {
            date: o
                .date
                .map(|d| (index(&FIELDS, &d.fields), index(&LENGTHS, &d.length))),
            time: o.time.map(|p| index(&PRECISIONS, &p)),
            zone: o
                .time_zone_style
                .filter(|_| o.time.is_some())
                .map(|z| index(&ZONE_STYLES, &z)),
        }
    }

    /// The options of this shape (no `hour12`, no calendar).
    pub fn options(self) -> DateTimeOptions<'static> {
        let mut o = DateTimeOptions::default();
        o.date = self.date.and_then(|(f, l)| {
            Some(DateStyle {
                fields: *FIELDS.get(usize::from(f))?,
                length: *LENGTHS.get(usize::from(l))?,
            })
        });
        o.time = self
            .time
            .and_then(|p| PRECISIONS.get(usize::from(p)).copied());
        o.time_zone_style = self
            .zone
            .and_then(|z| ZONE_STYLES.get(usize::from(z)).copied());
        o
    }

    /// Every shape `:datetime`, `:date` and `:time` can show: a date alone
    /// (`:date`), a time alone with or without a zone style (`:time`), both
    /// (`:datetime`).
    pub fn all() -> impl Iterator<Item = Shape> {
        let dates = (0..6u8).flat_map(|f| (0..3u8).map(move |l| Some((f, l))));
        let times = (0..3u8).flat_map(|p| [None, Some(0u8), Some(1)].map(move |z| (Some(p), z)));
        let date_only = dates.clone().map(|date| Shape {
            date,
            time: None,
            zone: None,
        });
        let time_only = times.clone().map(|(time, zone)| Shape {
            date: None,
            time,
            zone,
        });
        let both = dates.flat_map(move |date| {
            times
                .clone()
                .map(move |(time, zone)| Shape { date, time, zone })
        });
        date_only.chain(time_only).chain(both)
    }
}

/// What a corpus formats with the date functions: the slicing rule of
/// `icu.blob` (`plans/02-catalog-format.md` §4.4), decided by the build
/// from the expressions and literal options the corpus uses
/// ([`DateNeeds::add_message`]). Build one with `default()` (nothing) or
/// [`DateNeeds::all`], and add.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct DateNeeds {
    /// The shapes formatted.
    pub shapes: BTreeSet<Shape>,
    /// `hour12=true` may occur, `hour12=false` may occur (a literal, or a
    /// variable value: both). The locale's own hour cycle always may.
    pub hour12: [bool; 2],
    /// Calendars besides the locale's default and `gregory`: the literal
    /// `calendar=` values, or every calendar ([`CALENDARS`]) when a
    /// variable sets one (correct output over size, as for currencies and
    /// units). One that only an argument carries is not in the corpus
    /// (formatting it is an *Unsupported Operation*): configure it.
    pub calendars: Selection,
}

/// Every calendar a `calendar=` option can name that ICU4X 2.3 formats: the
/// BCP 47 `ca` values of CLDR 48 (those ICU4X does not build are skipped,
/// and formatting them is an *Unsupported Operation*).
pub const CALENDARS: [&str; 18] = [
    "buddhist",
    "chinese",
    "coptic",
    "dangi",
    "ethioaa",
    "ethiopic",
    "gregory",
    "hebrew",
    "indian",
    "islamic",
    "islamic-civil",
    "islamic-rgsa",
    "islamic-tbla",
    "islamic-umalqura",
    "iso8601",
    "japanese",
    "persian",
    "roc",
];

impl DateNeeds {
    /// Every shape, both `hour12` values, no calendar besides the locale's
    /// default and `gregory`.
    pub fn all() -> DateNeeds {
        DateNeeds {
            shapes: Shape::all().collect(),
            hour12: [true, true],
            calendars: Selection::default(),
        }
    }

    /// Whether nothing is formatted (no `icu.blob`).
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Adds what `message` formats: each `:datetime`, `:date` and `:time`
    /// expression's shape from its literal options (a variable or invalid
    /// value takes the default, as at run time), its `hour12` (a variable:
    /// both values) and its literal `calendar`; and `:datetime`'s defaults
    /// for a placeholder or declaration whose variable has no function and
    /// is not declared with one — it can receive a date/time argument,
    /// which `Registry::with_dates` formats (as `number.symbols` for a
    /// number, `syntax.json` #90). `hour12` and the calendars apply to every
    /// shape of the message, so what an expression inherits from its
    /// operand is covered.
    pub fn add_message(&mut self, message: &Message<'_>) {
        let mut declared_with_function = BTreeSet::new();
        let mut expressions: Vec<&Expression<'_>> = Vec::new();
        for d in message.declarations() {
            match d {
                Declaration::Input(i) => {
                    if i.value.function.is_some() {
                        declared_with_function.insert(i.value.arg.name.as_ref());
                    }
                }
                Declaration::Local(l) => {
                    if l.value.function().is_some() {
                        declared_with_function.insert(l.name.as_ref());
                    }
                    expressions.push(&l.value);
                }
                _ => {}
            }
        }
        let patterns: Vec<&Pattern<'_>> = match message {
            Message::Pattern(p) => vec![&p.pattern],
            Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
            _ => Vec::new(),
        };
        for p in patterns {
            for part in p {
                if let PatternPart::Expression(e) = part {
                    expressions.push(e);
                }
            }
        }
        for d in message.declarations() {
            if let Declaration::Input(i) = d {
                self.add_function(i.value.function.as_ref());
            }
        }
        for e in expressions {
            match (e.function(), e) {
                (Some(f), _) => self.add_function(Some(f)),
                (None, Expression::Variable(v))
                    if !declared_with_function.contains(v.arg.name.as_ref()) =>
                {
                    if let Some(o) = literal_options(None, &|_| None) {
                        self.shapes.insert(Shape::of(&o));
                    }
                }
                _ => {}
            }
        }
    }

    fn add_function(&mut self, f: Option<&FunctionRef<'_>>) {
        let Some(f) = f else {
            return;
        };
        let literal = |name: &str| match f.options.get(name) {
            Some(OptionValue::Literal(l)) => Some(l.value.as_ref()),
            _ => None,
        };
        let Some(o) = literal_options(Some(f.name.as_ref()), &literal) else {
            return;
        };
        self.shapes.insert(Shape::of(&o));
        match (o.hour12, f.options.get("hour12")) {
            (Some(true), _) => self.hour12[0] = true,
            (Some(false), _) => self.hour12[1] = true,
            (None, Some(OptionValue::Variable(_))) => self.hour12 = [true, true],
            (None, _) => {}
        }
        match (o.calendar, f.options.get("calendar")) {
            (Some(c), _) if c != "gregory" => self.calendars.add(c),
            (None, Some(OptionValue::Variable(_))) => self.calendars = Selection::All,
            _ => {}
        }
    }

    /// The calendars besides the locale's default and `gregory`.
    fn extra_calendars(&self) -> Vec<&str> {
        match &self.calendars {
            Selection::Listed(set) => set.iter().map(String::as_str).collect(),
            Selection::All => CALENDARS
                .iter()
                .copied()
                .filter(|c| *c != "gregory")
                .collect(),
        }
    }

    /// Every option set to build: each shape, under the locale's hour
    /// cycle and each `hour12` value that may occur, in the locale's
    /// default calendar, `gregory` and each extra calendar.
    fn expanded(&self) -> Vec<DateTimeOptions<'_>> {
        let mut hour12 = vec![None];
        if self.hour12[0] {
            hour12.push(Some(true));
        }
        if self.hour12[1] {
            hour12.push(Some(false));
        }
        let mut calendars = vec![None, Some("gregory")];
        calendars.extend(self.extra_calendars().into_iter().map(Some));
        let mut out = Vec::new();
        for shape in &self.shapes {
            for &h in &hour12 {
                for &c in &calendars {
                    let mut o = shape.options();
                    o.hour12 = h;
                    o.calendar = c;
                    out.push(o);
                }
            }
        }
        out
    }
}

/// What an `icu.blob` is built for: the `datetime-icu` backend variants it
/// serves, and what the corpus formats.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct IcuBlobSpec {
    /// Any calendar (`Icu<AnyCalendar, …>`): the locale's default calendar
    /// and the extra ones, not only `gregory` (the Gregorian variant's data
    /// is carried too).
    pub any_calendar: bool,
    /// Zone styles (`Icu<_, WithZones>`): zone names, metazone periods and
    /// the IANA → BCP-47 zone ids, for the shapes with a zone style (the
    /// no-zone variant's data is carried too).
    pub zones: bool,
    /// What the corpus formats.
    pub needs: DateNeeds,
}

impl IcuBlobSpec {
    /// A spec: Gregorian or any calendar, with or without zones, for
    /// `needs`.
    pub fn new(any_calendar: bool, zones: bool, needs: DateNeeds) -> IcuBlobSpec {
        IcuBlobSpec {
            any_calendar,
            zones,
            needs,
        }
    }

    /// Every variant (any calendar, with zones) for `needs` — what
    /// `mf2::compile_str` and the conformance harness use, whose registry
    /// is the widest.
    pub fn every_variant(needs: DateNeeds) -> IcuBlobSpec {
        IcuBlobSpec::new(true, true, needs)
    }

    /// Every variant, every shape.
    pub fn all() -> IcuBlobSpec {
        IcuBlobSpec::every_variant(DateNeeds::all())
    }
}

/// Primes `variant` for `locale` into `rec`; the shapes that failed.
fn prime_into<C: 'static, Z: 'static>(
    rec: &Recorder,
    locale: &str,
    needs: &DateNeeds,
    shapes: &[DateTimeOptions<'_>],
    failed: &mut Vec<String>,
) where
    (C, Z): Variant,
{
    for (o, e) in prime::<C, Z>(rec, locale, shapes) {
        // A calendar ICU4X does not know (a corpus's `calendar=abcd`) has
        // no data to record: formatting reports it (Unsupported Operation).
        if o.calendar
            .is_some_and(|c| c != "gregory" && needs.extra_calendars().contains(&c))
        {
            continue;
        }
        failed.push(format!("{o:?}: {e:?}"));
    }
}

/// The `icu.blob` entry for `locale` under `spec`: the requests of every
/// variant `spec` covers (with zones and any calendar, the no-zone and
/// Gregorian variants too, so one blob serves every narrower backend) for
/// every shape `spec.needs` holds. Empty needs give an empty blob — the
/// build writes no entry then. Deterministic; cached per process.
pub fn icu_blob(locale: &str, spec: &IcuBlobSpec) -> Result<Vec<u8>, Error> {
    type Cache = Mutex<BTreeMap<(String, IcuBlobSpec), Vec<u8>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    // One build at a time, so threads asking for the same blob wait for
    // it rather than build it again.
    static BUILD: Mutex<()> = Mutex::new(());
    let key = (locale.to_owned(), spec.clone());
    let cache = CACHE.get_or_init(Cache::default);
    let cached = || cache.lock().ok().and_then(|c| c.get(&key).cloned());
    if let Some(b) = cached() {
        return Ok(b);
    }
    let _build = BUILD
        .lock()
        .map_err(|_| DataError::custom("blob build lock"))?;
    if let Some(b) = cached() {
        return Ok(b);
    }
    let rec = Recorder::default();
    let needs = &spec.needs;
    let shapes = needs.expanded();
    let mut failed = Vec::new();
    prime_into::<GregorianOnly, NoZones>(&rec, locale, needs, &shapes, &mut failed);
    if spec.zones {
        prime_into::<GregorianOnly, WithZones>(&rec, locale, needs, &shapes, &mut failed);
    }
    if spec.any_calendar {
        prime_into::<AnyCalendar, NoZones>(&rec, locale, needs, &shapes, &mut failed);
        if spec.zones {
            prime_into::<AnyCalendar, WithZones>(&rec, locale, needs, &shapes, &mut failed);
        }
    }
    if !failed.is_empty() {
        return Err(Error::IcuBlobBuild {
            locale: locale.to_owned(),
            count: failed.len(),
            first: failed.swap_remove(0),
        });
    }
    let seen = rec
        .seen
        .into_inner()
        .map_err(|_| DataError::custom("recorder lock"))?;
    let blob = if seen.is_empty() {
        Vec::new()
    } else {
        export(&seen)?
    };
    if let Ok(mut c) = cache.lock() {
        c.insert(key, blob.clone());
    }
    Ok(blob)
}

/// The recorded responses as a blob (`BlobExporter`, blob format v3).
fn export(
    seen: &BTreeMap<(DataMarkerInfo, DataIdentifierCow<'static>), Recorded>,
) -> Result<Vec<u8>, Error> {
    let sink = Shared::default();
    let mut exporter = BlobExporter::new_with_sink(Box::new(sink.clone()));
    let flush = |checksum: Option<u64>| {
        let mut m = FlushMetadata::default();
        m.checksum = checksum;
        m
    };
    let mut markers: Vec<(DataMarkerInfo, Option<u64>)> = Vec::new();
    for ((marker, id), r) in seen {
        if marker.is_singleton {
            exporter.flush_singleton(*marker, &r.payload, flush(r.checksum))?;
        } else {
            exporter.put_payload(*marker, id.as_borrowed(), &r.payload)?;
            match markers.last_mut() {
                Some((m, c)) if m == marker => *c = c.or(r.checksum),
                _ => markers.push((*marker, r.checksum)),
            }
        }
    }
    for (marker, checksum) in markers {
        exporter.flush(marker, flush(checksum))?;
    }
    exporter.close()?;
    drop(exporter);
    let blob = sink
        .0
        .lock()
        .map_err(|_| DataError::custom("blob buffer lock"))?
        .clone();
    Ok(blob)
}
