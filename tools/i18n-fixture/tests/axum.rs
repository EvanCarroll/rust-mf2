//! A plain Axum application, with no Leptos (Phase 10 D1): the generated
//! `Locale` as an extractor, a response formatted per `Accept-Language`
//! through `Locale::format`, and the catalogs `mf2::axum::catalog_routes`
//! serves once `install()` has run.

use std::pin::pin;
use std::task::{Context, Poll, Waker};

use axum::Router;
use axum::body::Body;
use axum::http::header::{ACCEPT_LANGUAGE, CACHE_CONTROL, COOKIE, LOCATION};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use mf2_i18n_fixture::{Locale, tr};
use tower::ServiceExt as _;

async fn plain(locale: Locale) -> String {
    locale.format(&tr!("plain"))
}

async fn greeting(locale: Locale) -> String {
    locale.format(&tr!("greeting", name = "Ada"))
}

fn app() -> Router {
    mf2_i18n_fixture::install();
    Router::new()
        .route("/plain", get(plain))
        .route("/greeting", get(greeting))
        .merge(mf2::axum::catalog_routes())
}

/// Every future here is ready without I/O: poll it until it is.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

/// `GET path` with `headers`: the status, the `Location`, and the body.
fn fetch(path: &str, headers: &[(&str, &str)]) -> (StatusCode, Option<String>, Vec<u8>) {
    let mut request = Request::builder().uri(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = request.body(Body::empty()).expect("a request");
    let response = block_on(app().oneshot(request)).expect("the router answers");
    let status = response.status();
    let location = response
        .headers()
        .get(LOCATION)
        .map(|v| v.to_str().expect("an ASCII location").to_owned());
    let body =
        block_on(axum::body::to_bytes(response.into_body(), usize::MAX)).expect("the body reads");
    (status, location, body.to_vec())
}

fn text(path: &str, headers: &[(&str, &str)]) -> String {
    let (status, _, body) = fetch(path, headers);
    assert_eq!(status, StatusCode::OK, "{path}");
    String::from_utf8(body).expect("UTF-8 text")
}

#[test]
fn a_handler_answers_in_the_readers_language() {
    let al = ACCEPT_LANGUAGE.as_str();
    assert_eq!(text("/plain", &[(al, "pl")]), "Zapisz");
    assert_eq!(text("/plain", &[(al, "en-US,en;q=0.8")]), "Save");
    // A regional variant, lower in the list, still finds its language.
    assert_eq!(text("/plain", &[(al, "de, pl-PL;q=0.7")]), "Zapisz");
    // Nothing the build has: the source language.
    assert_eq!(text("/plain", &[(al, "fr")]), "Save");
    assert_eq!(text("/plain", &[]), "Save");
    // The cookie outranks `Accept-Language`, as `Negotiator::default()`'s.
    assert_eq!(
        text("/plain", &[(COOKIE.as_str(), "mf2_locale=pl"), (al, "en")]),
        "Zapisz"
    );
    // An argument, in the reader's language.
    let polish = text("/greeting", &[(al, "pl")]);
    assert!(
        polish.starts_with("Czesc, ") && polish.contains("Ada"),
        "{polish}"
    );
}

#[test]
fn the_catalogs_are_served_once_installed() {
    let (status, location, _) = fetch("/i18n/pl", &[]);
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("a redirect names the catalog");
    assert!(
        location.starts_with("/i18n/pl.") && location.contains(".mf2b"),
        "{location}"
    );
    let request = Request::builder()
        .uri(&location)
        .body(Body::empty())
        .expect("a request");
    let response = block_on(app().oneshot(request)).expect("the router answers");
    assert_eq!(response.status(), StatusCode::OK);
    let cache = response
        .headers()
        .get(CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(cache.contains("immutable"), "{cache}");
    let (status, _, _) = fetch("/i18n/de", &[]);
    assert_eq!(status, StatusCode::NOT_FOUND);
}
