//! The reader's time zone on the server (Phase 8 A7; `plans/03-runtime.md`
//! §6.1): a date is shown in the zone its message names (`input`: the
//! value's own), else the reader's, else `Setup::with_time_zone`'s — and the
//! page says which zone it used.
//!
//! A test binary of its own, because `install` is process-wide and this one
//! installs a `Setup` whose zone is not UTC.

// The Leptos line under test (`plans/04-leptos-integration.md` §10).
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;

use std::sync::Arc;

use leptos::prelude::*;
use leptos::text_prop::TextProp;
use leptos_mf2::{
    CatalogPreload, DateTimeValue, RequestI18n, Setup, TrArgs, install, install_catalogs,
    provide_locale_in_zone, reader_time_zone, tr_args1,
};
use mf2::{
    Arg, ArgValue, Compiled, DateTime, Dir, FormatContext, Formatter, Function, NoErrors, Registry,
    TimeZone,
};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("datetime", &mf2::fn_datetime::DATETIME)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr)];

/// 2026-01-01T00:00:00Z.
const NEW_YEAR: i64 = 1_767_225_600_000;

const MESSAGE: &str = "{$when :datetime}";

/// The message, installed once for the whole binary, in a `Setup` whose zone
/// is Tokyo's; the catalog is also the server's store, so that the preload
/// link renders.
fn installed() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let compiled = mf2::compile_str(MESSAGE, "en").expect("the message compiles");
        let hash = compiled.catalog.manifest_hash();
        let bytes: &'static [u8] = Vec::leak(compiled.catalog.into_bytes());
        install(
            Setup::new(&REGISTRY, &mf2::host_std::HOST, hash, "en", LOCALES)
                .with_time_zone(zone("Asia/Tokyo")),
        );
        install_catalogs(&[("en", "en.test.mf2b", bytes)]).expect("the catalog loads");
    });
}

fn zone(name: &str) -> TimeZone {
    TimeZone::named(name).expect("a zone name")
}

fn when() -> TrArgs {
    let value = DateTimeValue::instant(NEW_YEAR).expect("an instant");
    tr_args1(Compiled::ID, ArgValue::from(value))
}

/// Renders `body` as a request in the reader's zone `reader`.
fn in_request<R>(reader: Option<&str>, body: impl FnOnce() -> R) -> R {
    installed();
    Owner::new().with(|| {
        provide_locale_in_zone("en", reader.map(zone));
        body()
    })
}

/// What the runtime itself formats `source` to over the instant in `zone`:
/// the expected text, whichever date backend the build has (a workspace
/// build unifies one in).
fn formatted(source: &str, zone: TimeZone) -> String {
    installed();
    let compiled = mf2::compile_str(source, "en").expect("compiles");
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.time_zone = zone;
    let f = Formatter::new(&compiled.catalog, &REGISTRY, &cx);
    let mut out = String::new();
    let when = DateTime::from_epoch_ms(NEW_YEAR).expect("an instant");
    f.write(
        Compiled::ID,
        &[Arg::DateTime(&when)],
        &mut out,
        &mut NoErrors,
    );
    out
}

fn in_zone(name: &str) -> String {
    formatted(MESSAGE, zone(name))
}

#[test]
fn the_zones_this_test_uses_give_different_text() {
    let texts = ["Asia/Tokyo", "America/New_York", "Asia/Kolkata", "UTC"].map(in_zone);
    for (i, a) in texts.iter().enumerate() {
        for b in texts.iter().skip(i + 1) {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn without_the_readers_zone_a_date_is_in_setups() {
    assert_eq!(in_request(None, || when().to_html()), in_zone("Asia/Tokyo"));
}

#[test]
fn the_readers_zone_outranks_setups() {
    let ny = in_request(Some("America/New_York"), || when().to_html());
    assert_eq!(ny, in_zone("America/New_York"));
    let kolkata = in_request(Some("Asia/Kolkata"), || when().to_html());
    assert_eq!(kolkata, in_zone("Asia/Kolkata"));
}

/// Renders `source`'s message over the instant, for a reader in New York.
fn for_new_york(source: &str) -> String {
    installed();
    let catalog = Arc::new(mf2::compile_str(source, "en").expect("compiles").catalog);
    Owner::new().with(|| {
        RequestI18n::new(catalog)
            .with_time_zone(zone("America/New_York"))
            .provide();
        when().to_html()
    })
}

#[test]
fn the_zone_a_message_names_outranks_the_readers() {
    let named = "{$when :datetime timeZone=|Asia/Kolkata|}";
    assert_eq!(for_new_york(named), in_zone("Asia/Kolkata"));
    // `input`: the value's own zone, which for an instant is UTC.
    let input = "{$when :datetime timeZone=input}";
    assert_eq!(for_new_york(input), formatted(input, TimeZone::UTC));
    assert_eq!(for_new_york(input), in_zone("UTC"));
    assert_eq!(for_new_york(MESSAGE), in_zone("America/New_York"));
}

#[test]
fn a_conversion_keeps_the_requests_zone_when_read_later() {
    // `leptos_meta` reads a `TextProp` after rendering, outside the request
    // owner (04 §5): the conversion captured the zone with the catalog.
    let text: TextProp = in_request(Some("America/New_York"), || when().into());
    assert_eq!(text.get(), in_zone("America/New_York"));
}

#[test]
fn the_page_states_the_readers_zone_and_only_that() {
    let link = |reader| in_request(reader, || view! { <CatalogPreload /> }.to_html());
    let stated = link(Some("America/New_York"));
    assert!(
        stated.contains(r#"data-mf2-zone="America/New_York""#),
        "{stated}"
    );
    assert!(stated.contains("en.test.mf2b"), "{stated}");
    // In `Setup`'s zone the page says nothing: the client has the same Setup.
    let silent = link(None);
    assert!(!silent.contains("data-mf2-zone"), "{silent}");
}

#[test]
fn a_reader_zone_must_be_well_formed_and_known() {
    installed();
    assert!(reader_time_zone("America/New_York").is_some());
    assert!(reader_time_zone("UTC").is_some());
    for bad in [
        "",
        "Mars/Olympus_Mons",
        "../etc/passwd",
        "a b",
        "é",
        &"A".repeat(65),
    ] {
        assert!(reader_time_zone(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn a_request_without_a_zone_formats_as_before() {
    // `RequestI18n::new` alone: the path every existing caller takes.
    installed();
    let catalog = Arc::new(mf2::compile_str(MESSAGE, "en").expect("compiles").catalog);
    let html = Owner::new().with(|| {
        RequestI18n::new(catalog).provide();
        when().to_html()
    });
    assert_eq!(html, in_zone("Asia/Tokyo"));
}
