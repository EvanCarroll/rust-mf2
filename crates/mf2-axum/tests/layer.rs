//! The `Negotiator` as a tower layer (`plans/04-leptos-integration.md`
//! §12.5): a Leptos render finds its answer through the request `Parts`,
//! with no `_with_context` wiring, and the response says the language only
//! when something read it. With the `mf2_tz` cookie (Phase 8 A7), as
//! `time_zone.rs` checks it for 1.x's request glue: a zone this server knows
//! renders the request's dates in it and is stated on the preload link; a
//! malformed or unknown one is ignored.

// The Leptos line under test (`plans/04-leptos-integration.md` §10).
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;

use std::convert::Infallible;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use http::header::VARY;
use leptos::prelude::*;
use leptos_mf2::{CatalogPreload, DateTimeValue, Setup, tr_args1};
use mf2::{
    Arg, ArgValue, Compiled, DateTime, Dir, FormatContext, Formatter, Function, NoErrors, Registry,
    TimeZone,
};
use mf2_axum::{AcceptLanguage, Negotiator};
use tower::{Layer, ServiceExt, service_fn};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("datetime", &mf2::fn_datetime::DATETIME)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr)];

fn installed() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let compiled = mf2::compile_str("{$when :datetime}", "en").expect("the message compiles");
        let hash = compiled.catalog.manifest_hash();
        let bytes: &'static [u8] = Vec::leak(compiled.catalog.into_bytes());
        leptos_mf2::install(Setup::new(
            &REGISTRY,
            &mf2::host_std::HOST,
            hash,
            "en",
            LOCALES,
        ));
        leptos_mf2::install_catalogs(&[("en", "en.test.mf2b", bytes)]).expect("the catalog loads");
    });
}

/// What one request with `cookie` renders under the `negotiator` layer, as
/// a Leptos route does with the request `Parts` in its context: the date
/// (2026-01-01T00:00Z), the preload link, and `Vary`.
fn request(negotiator: &Negotiator, cookie: Option<&str>) -> (String, String, Vec<String>) {
    installed();
    let mut builder = http::Request::builder().uri("/");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    let request = builder.body(()).expect("a request");
    let route = service_fn(|request: http::Request<()>| async move {
        let parts = request.into_parts().0;
        let page = Owner::new().with(|| {
            provide_context(parts);
            let when = DateTimeValue::instant(1_767_225_600_000).expect("an instant");
            let date = tr_args1(Compiled::ID, ArgValue::from(when)).to_html();
            let link = view! { <CatalogPreload /> }.to_html();
            (date, link)
        });
        Ok::<_, Infallible>(http::Response::new(page))
    });
    let response = ready(negotiator.layer(route).oneshot(request)).expect("infallible");
    let vary = response
        .headers()
        .get_all(VARY)
        .iter()
        .filter_map(|v| v.to_str().ok().map(String::from))
        .collect();
    let (date, link) = response.into_body();
    (date, link, vary)
}

/// The output of a future that never waits: the layer and the route above.
fn ready<F: Future>(future: F) -> F::Output {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the request waited"),
    }
}

/// What the runtime itself formats the message to in `zone`: the expected
/// text, whichever date backend the build has.
fn in_zone(zone: TimeZone) -> String {
    let compiled = mf2::compile_str("{$when :datetime}", "en").expect("compiles");
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.time_zone = zone;
    let f = Formatter::new(&compiled.catalog, &REGISTRY, &cx);
    let mut out = String::new();
    let when = DateTime::from_epoch_ms(1_767_225_600_000).expect("an instant");
    f.write(
        Compiled::ID,
        &[Arg::DateTime(&when)],
        &mut out,
        &mut NoErrors,
    );
    out
}

fn by_header() -> Negotiator {
    Negotiator::over(LOCALES, "en").source(AcceptLanguage)
}

#[test]
fn the_cookie_sets_the_requests_zone() {
    let (date, link, vary) = request(&by_header(), Some("a=1; mf2_tz=America/New_York; b=2"));
    let ny = in_zone(TimeZone::named("America/New_York").expect("a zone"));
    assert_eq!(date, ny);
    assert_ne!(ny, in_zone(TimeZone::UTC));
    assert!(
        link.contains(r#"data-mf2-zone="America/New_York""#),
        "{link}"
    );
    // The page depends on the cookie now, and says so.
    assert!(vary.iter().any(|v| v == "cookie"), "{vary:?}");
}

#[test]
fn a_malformed_or_unknown_zone_is_ignored() {
    for cookie in [
        "mf2_tz=Mars/Olympus_Mons",
        "mf2_tz=../../etc/passwd",
        "mf2_tz=",
        "mf2_tz=%2FUTC",
        "mf2_tz=New York",
    ] {
        let (date, link, vary) = request(&by_header(), Some(cookie));
        assert_eq!(date, in_zone(TimeZone::UTC), "{cookie}");
        assert!(!link.contains("data-mf2-zone"), "{cookie}: {link}");
        assert!(!vary.iter().any(|v| v == "cookie"), "{cookie}: {vary:?}");
    }
}

#[test]
fn without_the_cookie_nothing_changes() {
    let (date, link, vary) = request(&by_header(), None);
    assert_eq!(date, in_zone(TimeZone::UTC));
    assert!(!link.contains("data-mf2-zone"), "{link}");
    assert_eq!(vary, ["accept-language"]);
}

#[test]
fn vary_names_the_cookie_once() {
    // The locale cookie already makes the response vary on `Cookie`. (The
    // default negotiator reads the installed locales, so install first.)
    installed();
    let (_, _, vary) = request(&Negotiator::default(), Some("mf2_tz=Asia/Tokyo"));
    let cookie = vary
        .iter()
        .flat_map(|v| v.split(','))
        .filter(|n| n.trim() == "cookie")
        .count();
    assert_eq!(cookie, 1, "{vary:?}");
}

#[test]
fn a_response_that_read_no_language_says_none() {
    // A static file or a catalog under the layer: nothing read the answer.
    installed();
    let request = http::Request::builder()
        .uri("/pkg/app.wasm?lang=en")
        .header("accept-language", "en")
        .body(())
        .expect("a request");
    let route =
        service_fn(|_: http::Request<()>| async { Ok::<_, Infallible>(http::Response::new(())) });
    let response = ready(Negotiator::default().layer(route).oneshot(request)).expect("infallible");
    assert!(response.headers().is_empty(), "{:?}", response.headers());
}

#[test]
fn a_page_says_its_language_and_remembers_a_choice() {
    installed();
    let (_, _, vary) = request(&Negotiator::default(), None);
    assert_eq!(vary, ["cookie, accept-language"]);
    // The query is an explicit choice: the cookie sink writes it.
    let request = http::Request::builder()
        .uri("/?lang=en")
        .body(())
        .expect("a request");
    let route = service_fn(|request: http::Request<()>| async move {
        let parts = request.into_parts().0;
        let tag = Owner::new().with(|| {
            provide_context(parts);
            mf2_axum::negotiated().map(|n| n.tag)
        });
        Ok::<_, Infallible>(http::Response::new(tag))
    });
    let response = ready(Negotiator::default().layer(route).oneshot(request)).expect("infallible");
    assert_eq!(response.headers()["content-language"], "en");
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .is_ok_and(|c| c.starts_with("mf2_locale=en"))
    );
    assert_eq!(response.into_body(), Some("en"));
}
