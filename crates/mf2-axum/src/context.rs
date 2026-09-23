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

use crate::negotiate::{Negotiated, Negotiator};

/// Negotiates this request's locale, provides its catalog, and sets
/// `Content-Language`, `Vary` and every sink's header on the response.
///
/// Returns what was negotiated, which the shell reads for `<html lang dir>`.
/// With no request in context — route-list generation, the file handler in a
/// bare owner — the default locale is used and no header is set.
pub fn provide_locale(negotiator: &Negotiator) -> Negotiated {
    let negotiated = match use_context::<Parts>() {
        Some(parts) => negotiator.negotiate(&parts),
        None => negotiator.negotiate(&mock_parts()),
    };
    // The catalog, for every `Tr` this request renders.
    leptos_mf2::provide_locale(negotiated.tag);
    // The answer, serialized for the shell to read.
    provide_context(negotiated.clone());

    if let Some(response) = use_context::<ResponseOptions>() {
        if let Ok(value) = HeaderValue::from_str(negotiated.tag) {
            response.insert_header(CONTENT_LANGUAGE, value);
        }
        if let Some(vary) = negotiator.vary() {
            response.append_header(VARY, vary);
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
