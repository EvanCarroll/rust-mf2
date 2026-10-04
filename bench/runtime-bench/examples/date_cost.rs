//! What a date placeholder costs with the `icu` date formatter (Phase 4,
//! A6; plans/03-runtime.md §5.2): per format, the catalog's `icu.blob` is
//! copied into an ICU4X `BlobDataProvider` and the formatter is built from
//! it — twice, since the runtime asks `formattable` before `format` — then
//! the value is formatted. Against the neutral stub backend (the semantics
//! alone). Native, one-message catalogs from `mf2::compile_str`, a reused
//! `String`, best of 9 × 2,000 calls. Timings on a machine whose clock
//! drifts: compare builds by alternating their binaries.
//!
//! By default the `icu` rows go through the formatter cache (`CachedBlob`,
//! plan/08 §5.2: the provider kept per catalog, the formatter per language
//! and shape); with the bench's `uncached-dates` feature, through `Blob`,
//! which builds both for every placeholder (the behaviour before the cache).
//!
//! ```sh
//! cargo run --release -p runtime-bench --example date_cost
//! cargo run --release -p runtime-bench --example date_cost --features uncached-dates
//! ```

use std::hint::black_box;
use std::time::Instant;

use mf2::{Arg, Compiled, DateTime, FormatContext, Formatter, Function, NoErrors, Registry};
#[cfg(feature = "uncached-dates")]
use mf2_fn_datetime::icu::Blob as IcuData;
#[cfg(not(feature = "uncached-dates"))]
use mf2_fn_datetime::icu::CachedBlob as IcuData;
use mf2_fn_datetime::icu::{AnyCalendar, Icu, WithZones};
use mf2_fn_datetime::{DateTimeFunction, Neutral};

/// The default backend's variant, with the data the build chose.
type IcuBackend = Icu<AnyCalendar, WithZones, IcuData>;

static I_DATE: DateTimeFunction<IcuBackend> = DateTimeFunction::date(Icu::NEW);
static I_DATETIME: DateTimeFunction<IcuBackend> = DateTimeFunction::datetime(Icu::NEW);
static I_TIME: DateTimeFunction<IcuBackend> = DateTimeFunction::time(Icu::NEW);
static ICU: [(&str, &dyn Function); 3] = [
    ("date", &I_DATE),
    ("datetime", &I_DATETIME),
    ("time", &I_TIME),
];
static ICU_REGISTRY: Registry = Registry::new(&ICU);

static N_DATE: DateTimeFunction<Neutral> = DateTimeFunction::date(Neutral);
static N_DATETIME: DateTimeFunction<Neutral> = DateTimeFunction::datetime(Neutral);
static N_TIME: DateTimeFunction<Neutral> = DateTimeFunction::time(Neutral);
static NEUTRAL: [(&str, &dyn Function); 3] = [
    ("date", &N_DATE),
    ("datetime", &N_DATETIME),
    ("time", &N_TIME),
];
static NEUTRAL_REGISTRY: Registry = Registry::new(&NEUTRAL);

static CX: FormatContext = FormatContext::new(&mf2::host_std::ZONES_HOST);

#[allow(clippy::cast_precision_loss)]
fn time(label: &str, registry: &Registry, src: &str, locale: &str) {
    let compiled: Compiled = mf2::compile_str(src, locale).expect("the message compiles");
    let formatter = Formatter::new(&compiled.catalog, registry, &CX);
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).expect("an instant");
    let args = [Arg::DateTime(&instant)];
    let mut out = String::with_capacity(256);
    let mut best = f64::MAX;
    let calls: u32 = 2_000;
    for _ in 0..9 {
        let start = Instant::now();
        for _ in 0..calls {
            out.clear();
            formatter.write(
                black_box(Compiled::ID),
                black_box(&args),
                &mut out,
                &mut NoErrors,
            );
        }
        best = best.min(start.elapsed().as_nanos() as f64 / f64::from(calls));
    }
    let blob = compiled
        .catalog
        .locale_entry(mf2_catalog::format::locale_key::ICU_BLOB)
        .map_or(0, <[u8]>::len);
    println!(
        "{:9.2} µs  {label:<44} blob {blob:>6} B -> {out:?}",
        best / 1000.0
    );
}

fn main() {
    let data = if cfg!(feature = "uncached-dates") {
        "Blob (uncached)"
    } else {
        "CachedBlob"
    };
    println!("icu data: {data}");
    for (src, locale) in [
        ("{$d :datetime}", "en"),
        ("{$d :date length=long}", "ja"),
        ("{$d :time precision=second hour12=false}", "de"),
        (
            "{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}",
            "en",
        ),
    ] {
        time(
            &format!("icu     {locale} {src}"),
            &ICU_REGISTRY,
            src,
            locale,
        );
        time(
            &format!("neutral {locale} {src}"),
            &NEUTRAL_REGISTRY,
            src,
            locale,
        );
    }
}
