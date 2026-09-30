//! The per-request locale, installed from inside the render
//! (`plans/04-leptos-integration.md` §5, §6).
//!
//! `leptos_axum` already provides the request's [`Parts`] and a
//! [`ResponseOptions`] in the reactive context, so negotiation, the catalog
//! and the response headers all happen in one place — the `additional_context`
//! closure — and no tower layer is needed for the application's own routes.
//!
//! **Pass it to every `_with_context` entry point.** There are three in
//! practice, and a missed one is a page rendered in the default locale with
//! no other symptom:
//!
//! ```ignore
//! let context = { let n = negotiator.clone(); move || { mf2_axum::provide_locale(&n); } };
//! let routes = generate_route_list_with_exclusions_and_context(App, None, context.clone());
//! let app = Router::new()
//!     .merge(mf2_axum::catalog_routes())
//!     .leptos_routes_with_context(&options, routes, context.clone(), {
//!         let options = options.clone();
//!         move || shell(options.clone())
//!     })
//!     .fallback(file_and_error_handler_with_context(context, shell));
//! ```
//!
//! Route-list generation runs the application with **mock** request parts and
//! the file handler calls it in a bare owner (P0.2), so everything here
//! tolerates a request that is not really there: with no `Parts` the default
//! locale is used, and nothing panics.

use http::header::{CONTENT_LANGUAGE, HeaderValue, VARY};
use http::request::Parts;
use leptos::prelude::{provide_context, use_context};
use leptos_axum::ResponseOptions;

use http::header::COOKIE;
use mf2::axum::{Negotiated, Negotiator};

/// Negotiates this request's locale, provides its catalog, and sets
/// `Content-Language`, `Vary` and every sink's header on the response.
///
/// It also reads the reader's time zone from the `mf2_tz` cookie the client
/// writes (`plans/03-runtime.md` §6.1): a zone this server knows renders the
/// request's dates in it, and the page says so; a malformed or unknown one
/// is ignored. Without `leptos-mf2`'s `fn-datetime` feature no zone is read.
///
/// Returns what was negotiated, which the shell reads for `<html lang dir>`.
/// With no request in context — route-list generation, the file handler in a
/// bare owner — the default locale is used and no header is set.
pub fn provide_locale(negotiator: &Negotiator) -> Negotiated {
    let parts = use_context::<Parts>();
    let negotiated = match &parts {
        Some(parts) => negotiator.negotiate(parts),
        None => negotiator.negotiate(&mock_parts()),
    };
    // The reader's time zone, if the cookie names one this server can use.
    let zone = parts.as_ref().and_then(|parts| {
        cookie(parts, leptos_mf2::links::TIME_ZONE_COOKIE).and_then(leptos_mf2::reader_time_zone)
    });
    // The catalog, for every `Tr` this request renders, in the reader's zone.
    leptos_mf2::provide_locale_in_zone(negotiated.tag, zone);
    // The answer, serialized for the shell to read.
    provide_context(negotiated.clone());

    if let Some(response) = use_context::<ResponseOptions>() {
        if let Ok(value) = HeaderValue::from_str(negotiated.tag) {
            response.insert_header(CONTENT_LANGUAGE, value);
        }
        let vary = negotiator.vary();
        // A page rendered in the cookie's zone depends on `Cookie`.
        let cookie_named = vary
            .as_ref()
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(',')
                    .any(|n| n.trim().eq_ignore_ascii_case("cookie"))
            });
        if let Some(vary) = vary {
            response.append_header(VARY, vary);
        }
        if zone.is_some() && !cookie_named {
            response.append_header(VARY, HeaderValue::from_static("cookie"));
        }
        for (name, value) in negotiator.store(&negotiated) {
            response.append_header(name, value);
        }
    }
    negotiated
}

/// What this request negotiated, for a component that needs it — the shell's
/// `<html lang dir>`, a `<LocaleSwitcher>`, an `hreflang` block.
#[must_use]
pub fn negotiated() -> Option<Negotiated> {
    use_context::<Negotiated>()
}

/// Empty request parts: what route-list generation effectively has.
fn mock_parts() -> Parts {
    http::Request::new(()).into_parts().0
}

/// The value of the cookie `name` on the request, if it carries one.
fn cookie<'r>(parts: &'r Parts, name: &str) -> Option<&'r str> {
    let header = parts.headers.get(COOKIE).and_then(|v| v.to_str().ok())?;
    header.split(';').find_map(|pair| {
        let (n, value) = pair.trim_start().split_once('=')?;
        (n.trim() == name).then(|| value.trim())
    })
}
