//! `icu.blob` (plans/11 A6; plans/02-catalog-format.md §4.4, §4.9): the
//! slicing rule read off messages; the blobs of the panel's locales (and
//! `th`, whose default calendar is Buddhist) within B4's per-locale budgets
//! (plans/06-size-and-perf.md: ≤ 3 KB gz without zone names, ≤ 25 KB gz
//! with); determinism and the test vectors of §4.9. Sizes are printed
//! (`cargo test -p mf2-locale-data --features icu-blob --test icu_blob
//! sizes -- --nocapture`).

#![cfg(feature = "icu-blob")]

use std::io::Write;

use mf2_locale_data::Selection;
use mf2_locale_data::icu_blob::{DateNeeds, IcuBlobSpec, Shape, icu_blob};
use mf2_runtime::{DateFields, DateLength, DateStyle, DateTimeOptions, TimePrecision, ZoneStyle};

const LOCALES: [&str; 12] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy", "th",
];

/// gzip -9, as the budgets are stated.
fn gz(b: &[u8]) -> usize {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(b).unwrap();
    e.finish().unwrap().len()
}

/// FNV-1a 64 (the catalog's hash function, 02 §3), for the vectors.
fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn needs(src: &str) -> DateNeeds {
    let model = mf2_syntax::parse_model(src).message.unwrap();
    let mut n = DateNeeds::default();
    n.add_message(&model);
    n
}

fn shape(
    date: Option<(DateFields, DateLength)>,
    time: Option<TimePrecision>,
    zone: Option<ZoneStyle>,
) -> Shape {
    let mut o = DateTimeOptions::default();
    o.date = date.map(|(fields, length)| DateStyle { fields, length });
    o.time = time;
    o.time_zone_style = zone;
    Shape::of(&o)
}

const YMD_MEDIUM: Option<(DateFields, DateLength)> =
    Some((DateFields::YearMonthDay, DateLength::Medium));

#[test]
fn slicing_rule() {
    use TimePrecision::{Minute, Second};
    let datetime = shape(YMD_MEDIUM, Some(Minute), None);
    let date = shape(YMD_MEDIUM, None, None);
    for (src, shapes, hour12, calendars) in [
        ("{$d :date}", vec![date], [false, false], vec![]),
        (
            "{|2006-01-02| :time}",
            vec![shape(None, Some(Minute), None)],
            [false, false],
            vec![],
        ),
        // Every option the shape reads, hour12 and a calendar.
        (
            "{$d :datetime dateFields=weekday dateLength=long timePrecision=second \
             timeZoneStyle=short hour12=true calendar=japanese timeZone=|Europe/Paris|}",
            vec![shape(
                Some((DateFields::Weekday, DateLength::Long)),
                Some(Second),
                Some(ZoneStyle::Short),
            )],
            [true, false],
            vec!["japanese"],
        ),
        // A variable hour12: both values; a variable calendar: every one.
        (
            "{$d :time hour12=$h calendar=$c}",
            vec![shape(None, Some(Minute), None)],
            [true, true],
            vec!["*"],
        ),
        // A non-override option by a variable, an invalid value, an option
        // the function does not take: the default (Bad Option, or ignored).
        (
            "{$d :date length=$l} {$d :date fields=x} {$d :date timeZoneStyle=long hour12=false}",
            vec![date],
            [false, false],
            vec![],
        ),
        // `gregory` is always carried; a malformed calendar is ignored.
        (
            "{$d :date calendar=gregory} {$d :date calendar=x}",
            vec![date],
            [false, false],
            vec![],
        ),
        // A placeholder that can receive a date/time argument: :datetime's
        // defaults. Not a variable declared with a function.
        ("{$x}", vec![datetime], [false, false], vec![]),
        (
            ".local $y = {$x} {{{$y}}}",
            vec![datetime],
            [false, false],
            vec![],
        ),
        (
            ".input {$x :number} {{{$x}}}",
            vec![],
            [false, false],
            vec![],
        ),
        (
            ".local $d = {|2006-01-02| :date length=short} {{{$d} {$d :time}}}",
            vec![
                shape(
                    Some((DateFields::YearMonthDay, DateLength::Short)),
                    None,
                    None,
                ),
                shape(None, Some(Minute), None),
            ],
            [false, false],
            vec![],
        ),
        (
            ".input {$d :datetime hour12=false} .local $t = {$d :time precision=second} \
             {{{$t}}}",
            vec![datetime, shape(None, Some(Second), None)],
            [false, true],
            vec![],
        ),
        // In selectors' variants too.
        (
            ".input {$n :number} .match $n 1 {{{$d :date}}} * {{{|x|}}}",
            vec![date],
            [false, false],
            vec![],
        ),
        (
            "{$x :string} {|2006-01-02|} {:number}",
            vec![],
            [false, false],
            vec![],
        ),
    ] {
        let n = needs(src);
        let mut want = shapes.clone();
        want.sort();
        want.dedup();
        assert_eq!(n.shapes.iter().copied().collect::<Vec<_>>(), want, "{src}");
        assert_eq!(n.hour12, hour12, "{src}");
        let want = if calendars == ["*"] {
            Selection::All
        } else {
            Selection::Listed(calendars.iter().map(|c| (*c).to_owned()).collect())
        };
        assert_eq!(n.calendars, want, "{src}");
        assert_eq!(n.is_empty(), shapes.is_empty(), "{src}");
    }
    // Every shape: 18 dates, 9 times (3 precisions × no zone, long, short),
    // 162 of both.
    assert_eq!(DateNeeds::all().shapes.len(), 18 + 9 + 162);
    // A shape's round trip.
    for s in Shape::all() {
        assert_eq!(Shape::of(&s.options()), s);
    }
}

fn spec(any_calendar: bool, zones: bool, n: &DateNeeds) -> IcuBlobSpec {
    IcuBlobSpec::new(any_calendar, zones, n.clone())
}

#[test]
fn sizes() {
    let default = needs("{$x}");
    let all = DateNeeds::all();
    eprintln!(
        "icu.blob, raw / gzip -9 B: `:datetime` defaults only (Gregorian); every shape: Gregorian, \
         any calendar, Gregorian with zones, every variant"
    );
    for locale in LOCALES {
        let mut row = Vec::new();
        for (name, spec, budget) in [
            ("defaults", spec(false, false, &default), 3 * 1024),
            ("gregorian", spec(false, false, &all), 3 * 1024),
            ("any-calendar", spec(true, false, &all), 3 * 1024),
            ("gregorian+zones", spec(false, true, &all), 25 * 1024),
            ("all", IcuBlobSpec::all(), 25 * 1024),
        ] {
            let blob = icu_blob(locale, &spec).unwrap();
            let size = gz(&blob);
            assert!(size <= budget, "{locale} {name}: {size} B gz > {budget}");
            row.push(format!("{name} {} / {size}", blob.len()));
        }
        eprintln!("  {locale}: {}", row.join(", "));
    }
}

#[test]
fn deterministic_and_nested() {
    let all = DateNeeds::all();
    let a = icu_blob("en", &spec(false, false, &all)).unwrap();
    let b = icu_blob("en", &spec(false, false, &all)).unwrap();
    assert_eq!(a, b);
    // The narrower variants' data is in the wider blobs (what the
    // `datetime-icu` tests format from, blob against compiled data).
    let narrow = icu_blob("th", &spec(false, false, &all)).unwrap();
    let wide = icu_blob("th", &IcuBlobSpec::all()).unwrap();
    assert!(narrow.len() < wide.len());
    // A zone style only brings zone data when a shape has one.
    let no_zone_shape = icu_blob("en", &spec(true, true, &needs("{$d :datetime}"))).unwrap();
    assert_eq!(
        no_zone_shape,
        icu_blob("en", &spec(true, false, &needs("{$d :datetime}"))).unwrap()
    );
    // An extra calendar adds its data; one ICU4X does not know adds none
    // (formatting it is an Unsupported Operation).
    let mut japanese = all.clone();
    japanese.calendars = Selection::Listed(["japanese", "abcd"].map(String::from).into());
    let with = icu_blob("en", &spec(true, false, &japanese)).unwrap();
    assert!(with.len() > icu_blob("en", &spec(true, false, &all)).unwrap().len());
    // Every calendar (a variable `calendar=`): more still, within B4's
    // budget without zone names.
    let mut every = needs("{$d :date calendar=$c}");
    assert_eq!(every.calendars, Selection::All);
    let any = icu_blob("en", &spec(true, false, &every)).unwrap();
    eprintln!(
        "en, `:date calendar=$c`: {} B, {} B gz",
        any.len(),
        gz(&any)
    );
    every.calendars = Selection::default();
    assert!(any.len() > icu_blob("en", &spec(true, false, &every)).unwrap().len());
    // Nothing formatted: no blob.
    assert!(
        icu_blob("en", &spec(true, true, &DateNeeds::default()))
            .unwrap()
            .is_empty()
    );
}

/// The vectors of plans/02-catalog-format.md §4.9: size, FNV-1a 64 and the
/// leading bytes of `en`'s blobs (CLDR 48 as ICU4X 2.3 bakes it; the zones vector follows
/// `icu_time_data`, which a time-zone data patch release moves — 2.3.1 here).
#[test]
fn vectors() {
    for (src, any_calendar, zones, len, hash) in [
        ("{$x}", false, false, 499usize, 0xc726_446f_5289_ecf2_u64),
        (
            "{$d :date length=long}",
            false,
            false,
            394,
            0xad77_c5d1_446f_33b3,
        ),
        (
            "{$d :time timeZoneStyle=short}",
            false,
            true,
            16_857,
            0xfdbb_a65d_3af5_6feb,
        ),
    ] {
        let blob = icu_blob("en", &spec(any_calendar, zones, &needs(src))).unwrap();
        let head: Vec<String> = blob.iter().take(16).map(|b| format!("{b:02X}")).collect();
        eprintln!(
            "{src} ({}, {}): {} B, {} B gz, fnv {:016x}, {}",
            if any_calendar {
                "any calendar"
            } else {
                "Gregorian"
            },
            if zones { "zones" } else { "no zones" },
            blob.len(),
            gz(&blob),
            fnv(&blob),
            head.join(" ")
        );
        assert_eq!((blob.len(), fnv(&blob)), (len, hash), "{src}");
        // Blob format v3 (`BlobSchema::V003`, a postcard enum: variant 3).
        assert_eq!(blob.first(), Some(&0x03));
    }
}
