//! The ICU4X backend (`std-icu`): text from catalogs
//! `mf2::compile_str` built with their `icu.blob`, in every variant (any or
//! Gregorian-only calendar, with or without zone styles); the blob against
//! ICU4X's compiled data, which must agree byte for byte (the blob holds
//! exactly what the backend requests); the errors.

#![allow(clippy::unwrap_used, clippy::too_many_lines)]

use mf2::{
    Arg, BidiStrategy, Date, DateTime, Dir, FormatContext, FormatError, Formatter, Function, Part,
    PartSink, Registry, Time, TimeZone,
};
use mf2_fn_datetime::DateTimeFunction;
#[cfg(feature = "compiled-data")]
use mf2_fn_datetime::icu::Compiled;
use mf2_fn_datetime::icu::{AnyCalendar, Blob, CachedBlob, GregorianOnly, Icu, NoZones, WithZones};

use FormatError::UnsupportedOperation;

/// The full backend: any calendar, zone styles, the catalog's blob.
type Full = Icu;

static DATETIME: DateTimeFunction<Full> = DateTimeFunction::datetime(Icu::NEW);
static DATE: DateTimeFunction<Full> = DateTimeFunction::date(Icu::NEW);
static TIME: DateTimeFunction<Full> = DateTimeFunction::time(Icu::NEW);
static FULL: [(&str, &dyn Function); 3] =
    [("date", &DATE), ("datetime", &DATETIME), ("time", &TIME)];
static REGISTRY: Registry = Registry::new(&FULL);

/// The same over ICU4X's compiled data (this crate's `compiled-data`, a set
/// of `cargo xtask ci`).
#[cfg(feature = "compiled-data")]
type FromCompiled = Icu<AnyCalendar, WithZones, Compiled>;
#[cfg(feature = "compiled-data")]
static C_DATETIME: DateTimeFunction<FromCompiled> = DateTimeFunction::datetime(Icu::NEW);
#[cfg(feature = "compiled-data")]
static C_DATE: DateTimeFunction<FromCompiled> = DateTimeFunction::date(Icu::NEW);
#[cfg(feature = "compiled-data")]
static C_TIME: DateTimeFunction<FromCompiled> = DateTimeFunction::time(Icu::NEW);
#[cfg(feature = "compiled-data")]
static COMPILED: [(&str, &dyn Function); 3] = [
    ("date", &C_DATE),
    ("datetime", &C_DATETIME),
    ("time", &C_TIME),
];
#[cfg(feature = "compiled-data")]
static COMPILED_REGISTRY: Registry = Registry::new(&COMPILED);

/// The narrowest: Gregorian only, no zone styles.
type Narrow = Icu<GregorianOnly, NoZones>;
static N_DATETIME: DateTimeFunction<Narrow> = DateTimeFunction::datetime(Icu::NEW);
static N_DATE: DateTimeFunction<Narrow> = DateTimeFunction::date(Icu::NEW);
static N_TIME: DateTimeFunction<Narrow> = DateTimeFunction::time(Icu::NEW);
static NARROW: [(&str, &dyn Function); 3] = [
    ("date", &N_DATE),
    ("datetime", &N_DATETIME),
    ("time", &N_TIME),
];
static NARROW_REGISTRY: Registry = Registry::new(&NARROW);

/// The full backend with each data explicitly: the formatter cache and the
/// formatter built for each placeholder (the native host's default is the
/// cache, so `REGISTRY` is it too).
type Cached = Icu<AnyCalendar, WithZones, CachedBlob>;
type Uncached = Icu<AnyCalendar, WithZones, Blob>;
static K_DATE: DateTimeFunction<Cached> = DateTimeFunction::date(Icu::NEW);
static K_DATETIME: DateTimeFunction<Cached> = DateTimeFunction::datetime(Icu::NEW);
static K_TIME: DateTimeFunction<Cached> = DateTimeFunction::time(Icu::NEW);
static CACHED: [(&str, &dyn Function); 3] = [
    ("date", &K_DATE),
    ("datetime", &K_DATETIME),
    ("time", &K_TIME),
];
static CACHED_REGISTRY: Registry = Registry::new(&CACHED);
static U_DATE: DateTimeFunction<Uncached> = DateTimeFunction::date(Icu::NEW);
static U_DATETIME: DateTimeFunction<Uncached> = DateTimeFunction::datetime(Icu::NEW);
static U_TIME: DateTimeFunction<Uncached> = DateTimeFunction::time(Icu::NEW);
static UNCACHED: [(&str, &dyn Function); 3] = [
    ("date", &U_DATE),
    ("datetime", &U_DATETIME),
    ("time", &U_TIME),
];
static UNCACHED_REGISTRY: Registry = Registry::new(&UNCACHED);

/// Gregorian with zones, and any calendar without.
static GZ_TIME: DateTimeFunction<Icu<GregorianOnly, WithZones>> = DateTimeFunction::time(Icu::NEW);
static GZ_DATE: DateTimeFunction<Icu<GregorianOnly, WithZones>> = DateTimeFunction::date(Icu::NEW);
static AN_TIME: DateTimeFunction<Icu<AnyCalendar, NoZones>> = DateTimeFunction::time(Icu::NEW);
static AN_DATE: DateTimeFunction<Icu<AnyCalendar, NoZones>> = DateTimeFunction::date(Icu::NEW);
static GZ: [(&str, &dyn Function); 2] = [("date", &GZ_DATE), ("time", &GZ_TIME)];
static GZ_REGISTRY: Registry = Registry::new(&GZ);
static AN: [(&str, &dyn Function); 2] = [("date", &AN_DATE), ("time", &AN_TIME)];
static AN_REGISTRY: Registry = Registry::new(&AN);

#[cfg(feature = "compiled-data")]
const PANEL: [&str; 11] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

fn cx(zone: TimeZone) -> FormatContext {
    let mut cx = FormatContext::new(&mf2::host_std::ZONES_HOST);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = zone;
    cx
}

fn run_with(
    registry: &Registry,
    cx: &FormatContext,
    locale: &str,
    src: &str,
    args: &[(&str, Arg<'_>)],
) -> (String, Vec<FormatError>) {
    let m = mf2::compile_str(src, locale).unwrap_or_else(|e| panic!("{src}: {e}"));
    let f = Formatter::new(&m.catalog, registry, cx);
    let mut out = String::new();
    let mut errors = Vec::new();
    f.write_named(mf2::Compiled::ID, args, &mut out, &mut errors);
    (out, errors)
}

/// `src` in `locale` with the full backend, UTC; no errors allowed.
fn ok(locale: &str, src: &str) -> String {
    let (s, e) = run_with(&REGISTRY, &cx(TimeZone::UTC), locale, src, &[]);
    assert!(e.is_empty(), "{locale} {src}: {e:?} ({s:?})");
    s
}

#[test]
fn en_options() {
    for (src, want) in [
        (
            "{|2006-01-02T15:04:06| :datetime}",
            "Jan 2, 2006, 3:04\u{202f}PM",
        ),
        (
            "{|2006-01-02T15:04:06| :datetime dateLength=long timePrecision=second}",
            "January 2, 2006 at 3:04:06\u{202f}PM",
        ),
        (
            "{|2006-01-02T15:04:06| :datetime dateLength=short}",
            "1/2/06, 3:04\u{202f}PM",
        ),
        (
            "{|2006-01-02T15:04:06| :datetime dateFields=weekday timePrecision=hour hour12=true}",
            "Mon, 3\u{202f}PM",
        ),
        ("{|2006-01-02| :date}", "Jan 2, 2006"),
        ("{|2006-01-02| :date length=long}", "January 2, 2006"),
        ("{|2006-01-02| :date length=short}", "1/2/06"),
        ("{|2006-01-02| :date fields=weekday}", "Mon"),
        (
            "{|2006-01-02| :date fields=month-day length=long}",
            "January 2",
        ),
        (
            "{|2006-01-02| :date fields=year-month-day-weekday length=long}",
            "Monday, January 2, 2006",
        ),
        ("{|2006-01-02T15:04:06| :time}", "3:04\u{202f}PM"),
        (
            "{|2006-01-02T15:04:06| :time precision=second}",
            "3:04:06\u{202f}PM",
        ),
        (
            "{|2006-01-02T15:04:06| :time precision=hour}",
            "3\u{202f}PM",
        ),
        ("{|2006-01-02T15:04:06| :time hour12=false}", "15:04"),
        (
            "{|2006-01-02T00:30:00| :time hour12=true}",
            "12:30\u{202f}AM",
        ),
    ] {
        assert_eq!(ok("en-US", src), want, "{src}");
    }
    // The suite's own cases (functions/{date,time,datetime}.json).
    assert_eq!(
        ok(
            "en-US",
            ".local $d = {|2006-01-02| :date length=long} {{{$d}}}"
        ),
        "January 2, 2006"
    );
    assert_eq!(
        ok(
            "en-US",
            ".local $d = {|2006-01-02| :datetime dateLength=long timePrecision=second} {{{$d :date}}}"
        ),
        "Jan 2, 2006"
    );
    assert_eq!(
        ok(
            "en-US",
            ".local $t = {|2006-01-02T15:04:06| :datetime dateLength=long timePrecision=second} {{{$t :time}}}"
        ),
        "3:04\u{202f}PM"
    );
    assert_eq!(
        ok("und", "{|2006-01-02T15:04:06| :datetime}"),
        "2006 M01 2 15:04"
    );
}

#[test]
fn locales_and_calendars() {
    assert_eq!(
        ok(
            "fr",
            "{|2006-01-02T15:04:06| :datetime dateFields=year-month-day-weekday dateLength=long}"
        ),
        "lundi 2 janvier 2006 à 15:04"
    );
    assert_eq!(
        ok("ja", "{|2006-01-02T15:04:06| :datetime}"),
        "2006/01/02 15:04"
    );
    assert_eq!(
        ok("ja", "{|2006-01-02T15:04:06| :time hour12=true}"),
        "午後3:04"
    );
    // A locale whose default calendar is not Gregorian.
    assert_eq!(
        ok("th", "{|2006-01-02| :date length=long}"),
        "2 มกราคม 2549"
    );
    assert_eq!(
        ok("en-u-ca-buddhist", "{|2006-01-02| :date}"),
        "Jan 2, 2549 BE"
    );
    // `calendar=`: its data comes with the literal (compile_str).
    assert_eq!(
        ok(
            "en-US",
            "{|2006-01-02| :date calendar=japanese length=long}"
        ),
        "January 2, 18 Heisei"
    );
    assert_eq!(
        ok("en-US", "{|2006-01-02| :date calendar=hebrew}"),
        "2 Tevet 5766"
    );
    assert_eq!(
        ok("th", "{|2006-01-02| :date calendar=gregory}"),
        "2 ม.ค. 2006"
    );
    // Before the common era.
    let bce = DateTime::floating(Date::new(-44, 3, 15).unwrap(), Time::MIDNIGHT);
    let (s, e) = run_with(
        &REGISTRY,
        &cx(TimeZone::UTC),
        "en",
        "{$d :date length=long}",
        &[("d", Arg::DateTime(&bce))],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("March 15, 45 BC", &[][..]));
}

#[test]
fn zones() {
    let utc = cx(TimeZone::UTC);
    for (src, want) in [
        (
            "{|2006-01-02T15:04:06Z| :time timeZoneStyle=long}",
            "3:04\u{202f}PM Coordinated Universal Time",
        ),
        (
            "{|2006-01-02T15:04:06Z| :time timeZoneStyle=short}",
            "3:04\u{202f}PM UTC",
        ),
        (
            "{|2006-01-02T15:04:06Z| :time timeZoneStyle=short timeZone=|America/New_York|}",
            "10:04\u{202f}AM EST",
        ),
        (
            "{|2006-07-02T15:04:06Z| :datetime timeZoneStyle=long timeZone=|Europe/Paris|}",
            "Jul 2, 2006, 5:04\u{202f}PM Central European Summer Time",
        ),
        (
            "{|2006-01-02T15:04:06| :time timeZoneStyle=long timeZone=|+05:30|}",
            "3:04\u{202f}PM GMT+05:30",
        ),
        // The zone converts without a style too.
        (
            "{|2006-01-02T15:04:06Z| :time timeZone=|Asia/Kolkata|}",
            "8:34\u{202f}PM",
        ),
    ] {
        let (s, e) = run_with(&REGISTRY, &utc, "en-US", src, &[]);
        assert_eq!((s.as_str(), e.as_slice()), (want, &[][..]), "{src}");
    }
    // The context's zone, named: its name shows.
    let paris = cx(TimeZone::named("Europe/Paris").unwrap());
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    let (s, e) = run_with(
        &REGISTRY,
        &paris,
        "de",
        "{$i :time timeZoneStyle=long}",
        &[("i", Arg::DateTime(&instant))],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("16:04 Mitteleuropäische Normalzeit", &[][..])
    );
    // `:datetime`'s defaults, in the context's zone.
    let (s, e) = run_with(
        &REGISTRY,
        &paris,
        "en",
        "{$i :datetime} {|2006-01-02| :date}",
        &[("i", Arg::DateTime(&instant))],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("Jan 2, 2006, 4:04\u{202f}PM Jan 2, 2006", &[][..])
    );
}

#[test]
fn variants() {
    let utc = cx(TimeZone::UTC);
    let run = |r: &Registry, locale: &str, src: &str| run_with(r, &utc, locale, src, &[]);
    // Gregorian only: a locale's other default calendar gives way …
    assert_eq!(
        run(&NARROW_REGISTRY, "th", "{|2006-01-02| :date length=long}"),
        ("2 มกราคม ค.ศ. 2006".into(), vec![])
    );
    // … `calendar=gregory` is fine, any other an Unsupported Operation.
    assert_eq!(
        run(
            &NARROW_REGISTRY,
            "en",
            "{|2006-01-02| :date calendar=gregory}"
        ),
        ("Jan 2, 2006".into(), vec![])
    );
    assert_eq!(
        run(
            &NARROW_REGISTRY,
            "en",
            "{|2006-01-02| :date calendar=japanese}"
        ),
        ("{|2006-01-02|}".into(), vec![UnsupportedOperation])
    );
    // No zones: a zone style is an Unsupported Operation; the rest formats.
    assert_eq!(
        run(
            &NARROW_REGISTRY,
            "en",
            "{|2006-01-02T15:04:06| :time timeZoneStyle=long}"
        ),
        ("{|2006-01-02T15:04:06|}".into(), vec![UnsupportedOperation])
    );
    assert_eq!(
        run(
            &NARROW_REGISTRY,
            "en",
            "{|2006-01-02T15:04:06Z| :time timeZone=|Asia/Kolkata|}"
        ),
        ("8:34\u{202f}PM".into(), vec![])
    );
    assert_eq!(
        run(&NARROW_REGISTRY, "en", "{|2006-01-02T15:04:06| :datetime}"),
        ("Jan 2, 2006, 3:04\u{202f}PM".into(), vec![])
    );
    assert_eq!(
        run(
            &GZ_REGISTRY,
            "en",
            "{|2006-01-02T15:04:06Z| :time timeZoneStyle=short}"
        ),
        ("3:04\u{202f}PM UTC".into(), vec![])
    );
    assert_eq!(
        run(
            &AN_REGISTRY,
            "en",
            "{|2006-01-02T15:04:06| :time timeZoneStyle=short}"
        ),
        ("{|2006-01-02T15:04:06|}".into(), vec![UnsupportedOperation])
    );
    assert_eq!(
        run(&AN_REGISTRY, "th", "{|2006-01-02| :date}"),
        ("2 ม.ค. 2549".into(), vec![])
    );
    assert_eq!(
        run(&GZ_REGISTRY, "th", "{|2006-01-02| :date}"),
        ("2 ม.ค. 2006".into(), vec![])
    );
}

#[test]
fn errors() {
    let utc = cx(TimeZone::UTC);
    // A calendar ICU4X does not know.
    assert_eq!(
        run_with(
            &REGISTRY,
            &utc,
            "en",
            "{|2006-01-02| :date calendar=abcd}",
            &[]
        ),
        ("{|2006-01-02|}".into(), vec![UnsupportedOperation])
    );
    // A variable calendar: the catalog carries every calendar (02 §4.4) …
    assert_eq!(
        run_with(
            &REGISTRY,
            &utc,
            "en",
            "{|2006-01-02| :date calendar=$c}",
            &[("c", Arg::Str("japanese"))]
        ),
        ("Jan 2, 18 Heisei".into(), vec![])
    );
    // … but one an argument carries is not in the corpus.
    let mut japanese = DateTime::floating(Date::new(2006, 1, 2).unwrap(), Time::MIDNIGHT);
    japanese.options.calendar = Some("japanese");
    assert_eq!(
        run_with(
            &REGISTRY,
            &utc,
            "en",
            "{$d :date}",
            &[("d", Arg::DateTime(&japanese))]
        ),
        ("{$d}".into(), vec![UnsupportedOperation])
    );
    // A year outside ICU4X's range.
    let far = DateTime::floating(Date::new(999_999, 3, 15).unwrap(), Time::MIDNIGHT);
    assert_eq!(
        run_with(
            &REGISTRY,
            &utc,
            "en",
            "{$d :date}",
            &[("d", Arg::DateTime(&far))]
        ),
        ("{$d}".into(), vec![UnsupportedOperation])
    );
    // `:datetime` with no options: `compile_str` carries its defaults
    // (02 §4.4).
    let d = mf2_fn_datetime::parse_literal("2006-01-02T15:04:06").unwrap();
    assert_eq!(
        run_with(
            &REGISTRY,
            &utc,
            "en",
            "{$d :datetime}",
            &[("d", Arg::DateTime(&d))]
        ),
        ("Jan 2, 2006, 3:04\u{202f}PM".into(), vec![])
    );
    // A catalog without its blob.
    let model = mf2_syntax::parse_model("{$d :date}").message.unwrap();
    let options = mf2_catalog::writer::Options::new("en", mf2::Dir::Ltr);
    let (bytes, manifest) = mf2_catalog::writer::single(&model, &["d"], &options).unwrap();
    let catalog = mf2::Catalog::new(bytes, manifest.hash()).unwrap();
    let f = Formatter::new(&catalog, &REGISTRY, &utc);
    let (mut out, mut errors) = (String::new(), Vec::new());
    f.write_named(
        mf2::Compiled::ID,
        &[("d", Arg::DateTime(&d))],
        &mut out,
        &mut errors,
    );
    assert_eq!(
        (out.as_str(), errors.as_slice()),
        ("{$d}", &[UnsupportedOperation][..])
    );
    // Semantic errors are the semantics' (unchanged by the backend).
    assert_eq!(
        run_with(&REGISTRY, &utc, "en", "{horse :date}", &[]),
        ("{|horse|}".into(), vec![FormatError::BadOperand])
    );
}

#[test]
fn parts_and_direction() {
    struct Kinds(Vec<(String, Dir, String)>);
    impl PartSink for Kinds {
        fn part(&mut self, part: Part<'_>) {
            if let Part::Expression(e) = part {
                let mut s = String::new();
                e.write(&mut s);
                self.0.push((e.kind().to_owned(), e.dir(), s));
            }
        }
    }
    for (locale, dir) in [("en", Dir::Ltr), ("ar", Dir::Rtl), ("he", Dir::Rtl)] {
        let m = mf2::compile_str("{|2006-01-02| :date}", locale).unwrap();
        let cx = FormatContext::new(&mf2::host_std::ZONES_HOST);
        let f = Formatter::new(&m.catalog, &REGISTRY, &cx);
        let mut k = Kinds(Vec::new());
        f.parts(mf2::Compiled::ID, &[], &mut k, &mut Vec::new());
        assert_eq!(k.0.len(), 1);
        assert_eq!((k.0[0].0.as_str(), k.0[0].1), ("datetime", dir), "{locale}");
        // The Default Bidi Strategy: an LTR value in an LTR message stands
        // as it is, an RTL one is always isolated (formatting.md).
        let mut s = String::new();
        f.write(mf2::Compiled::ID, &[], &mut s, &mut Vec::new());
        let want = match dir {
            Dir::Rtl => format!("\u{2067}{}\u{2069}", k.0[0].2),
            _ => k.0[0].2.clone(),
        };
        assert_eq!(s, want, "{locale}");
    }
}

/// Every option shape, formatted from each panel locale's blob and from
/// ICU4X's compiled data: the same bytes.
#[cfg(feature = "compiled-data")]
#[test]
fn blob_equals_compiled_data() {
    let mut messages = Vec::new();
    for fields in [
        "weekday",
        "day-weekday",
        "month-day",
        "month-day-weekday",
        "year-month-day",
        "year-month-day-weekday",
    ] {
        for length in ["long", "medium", "short"] {
            messages.push(format!(
                "{{|2006-01-02T15:04:06Z| :datetime dateFields={fields} dateLength={length}}}"
            ));
            messages.push(format!(
                "{{|2006-01-02| :date fields={fields} length={length}}}"
            ));
        }
    }
    for precision in ["hour", "minute", "second"] {
        for zone in ["", " timeZoneStyle=long", " timeZoneStyle=short"] {
            for hour12 in ["", " hour12=true", " hour12=false"] {
                messages.push(format!(
                    "{{|2006-07-02T15:04:06Z| :time precision={precision}{zone}{hour12} timeZone=|America/New_York|}}"
                ));
            }
        }
    }
    messages.push(
        "{|2006-07-02T15:04:06Z| :datetime timePrecision=second timeZoneStyle=long timeZone=|Asia/Kolkata|}"
            .into(),
    );
    messages
        .push("{|2006-01-02T15:04:06Z| :datetime timeZoneStyle=short timeZone=|+05:45|}".into());
    let utc = cx(TimeZone::UTC);
    let mut compared = 0;
    for locale in PANEL {
        for src in &messages {
            let blob = run_with(&REGISTRY, &utc, locale, src, &[]);
            let compiled = run_with(&COMPILED_REGISTRY, &utc, locale, src, &[]);
            assert!(blob.1.is_empty(), "{locale} {src}: {:?}", blob.1);
            assert_eq!(blob, compiled, "{locale} {src}");
            assert!(
                !blob.0.is_empty(),
                "the blob for a locale with dates is empty"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 11 * messages.len());
}

/// The formatter cache (plan/08 §5.2): two catalogs of one language
/// formatted in turn each get their own formatter, and a catalog compiled
/// after another is dropped (a reload) is not served the dropped one's: the
/// text is the uncached backend's, every time.
#[test]
fn cache_two_catalogs_in_turn() {
    let utc = cx(TimeZone::UTC);
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    let args = [("d", Arg::DateTime(&instant))];
    let run = |registry: &Registry, m: &mf2::Compiled| {
        let f = Formatter::new(&m.catalog, registry, &utc);
        let mut out = String::new();
        let mut errors = Vec::new();
        f.write_named(mf2::Compiled::ID, &args, &mut out, &mut errors);
        assert!(errors.is_empty(), "{errors:?} ({out:?})");
        out
    };
    let compile = |src: &str| mf2::compile_str(src, "en").unwrap_or_else(|e| panic!("{src}: {e}"));
    let a = compile("{$d :date length=long}");
    let b = compile("{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}");
    let (want_a, want_b) = (run(&UNCACHED_REGISTRY, &a), run(&UNCACHED_REGISTRY, &b));
    assert_ne!(want_a, want_b);
    for _ in 0..3 {
        assert_eq!(run(&CACHED_REGISTRY, &a), want_a);
        assert_eq!(run(&CACHED_REGISTRY, &b), want_b);
    }
    drop(a);
    let c = compile("{$d :time precision=second hour12=false}");
    let want_c = run(&UNCACHED_REGISTRY, &c);
    assert_ne!(want_c, want_a);
    for _ in 0..3 {
        assert_eq!(run(&CACHED_REGISTRY, &c), want_c);
        assert_eq!(run(&CACHED_REGISTRY, &b), want_b);
    }
}

/// The cache knows a catalog by its load number (plan/08 §5.2): two
/// catalogs with byte-identical slices, loaded separately, each format as
/// the uncached backend does, and so does one loaded after another with the
/// same slice is dropped — whatever the cache kept for the dropped one.
#[test]
fn cache_keys_on_the_load_number() {
    let utc = cx(TimeZone::UTC);
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    let args = [("d", Arg::DateTime(&instant))];
    let run = |registry: &Registry, m: &mf2::Compiled| {
        let f = Formatter::new(&m.catalog, registry, &utc);
        let mut out = String::new();
        let mut errors = Vec::new();
        f.write_named(mf2::Compiled::ID, &args, &mut out, &mut errors);
        assert!(errors.is_empty(), "{errors:?} ({out:?})");
        out
    };
    let src = "{$d :datetime dateLength=long timeZone=|Europe/Warsaw| timeZoneStyle=long}";
    let compile = || mf2::compile_str(src, "pl").unwrap_or_else(|e| panic!("{src}: {e}"));
    let blob = |m: &mf2::Compiled| {
        m.catalog
            .locale_entry(mf2_catalog::format::locale_key::ICU_BLOB)
            .map(<[u8]>::to_vec)
    };
    let (a, b) = (compile(), compile());
    assert!(blob(&a).is_some());
    assert_eq!(blob(&a), blob(&b));
    assert_ne!(a.catalog.load_id(), b.catalog.load_id());
    let want = run(&UNCACHED_REGISTRY, &a);
    assert_eq!(run(&UNCACHED_REGISTRY, &b), want);
    for _ in 0..3 {
        assert_eq!(run(&CACHED_REGISTRY, &a), want);
        assert_eq!(run(&CACHED_REGISTRY, &b), want);
    }
    let gone = a.catalog.load_id();
    drop(a);
    let c = compile();
    assert_ne!(c.catalog.load_id(), gone);
    assert_ne!(c.catalog.load_id(), b.catalog.load_id());
    for _ in 0..3 {
        assert_eq!(run(&CACHED_REGISTRY, &c), want);
        assert_eq!(run(&CACHED_REGISTRY, &b), want);
    }
}
