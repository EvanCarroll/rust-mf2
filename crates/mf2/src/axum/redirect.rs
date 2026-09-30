//! A site whose locale is in its URLs (`/fr/…`), without client code: the
//! switcher's form submits `?lang=`, which a path prefix outranks, so the
//! server sends the reader to that language's URL instead (Phase 9 B4).

use ::axum::extract::{Request, State};
use ::axum::middleware::Next;
use ::axum::response::{IntoResponse, Redirect, Response};
use ::http::{Method, Uri};
use mf2_catalog::Dir;

use super::negotiate::lookup;
use alloc::string::String;
use alloc::vec::Vec;

/// Middleware for a site whose first path segment is the locale
/// ([`PathPrefix`](super::PathPrefix)): a `GET` whose `?<name>=` names
/// another locale than its path is redirected to the same path under that
/// locale, the parameter removed and every other one kept —
/// `/en/about?lang=fr` to `/fr/about`. That is what `<LocaleSwitcher>`'s form
/// submits before the wasm has loaded, or on a page without it. Anything
/// else passes through.
///
/// `name` is the query parameter the negotiator's query source reads
/// ([`Negotiator::query_name`](super::Negotiator::query_name)), given as the
/// middleware's state:
///
/// ```ignore
/// let query = negotiator.query_name().unwrap_or(mf2::axum::QueryParam::default().0);
/// let app = Router::new()
///     // …routes…
///     .layer(axum::middleware::from_fn_with_state(query, mf2::axum::path_prefix_redirect));
/// ```
///
/// The locales are the build's, from the generated `install()`.
pub async fn path_prefix_redirect(
    State(name): State<&'static str>,
    request: Request,
    next: Next,
) -> Response {
    if matches!(*request.method(), Method::GET | Method::HEAD)
        && let Some(to) = path_for_query(request.uri(), name, super::locales())
    {
        return Redirect::to(&to).into_response();
    }
    next.run(request).await
}

/// Where `uri` should go: its path under the locale `?<param>=` names, when
/// that differs from the locale its first segment names. `None` when either
/// is missing or unknown, or they agree.
pub(crate) fn path_for_query(
    uri: &Uri,
    param: &str,
    locales: &[(&'static str, Dir)],
) -> Option<String> {
    let rest = uri.path().strip_prefix('/')?;
    let (first, tail) = match rest.split_once('/') {
        Some((first, tail)) => (first, Some(tail)),
        None => (rest, None),
    };
    let (current, _) = lookup(first, locales)?;
    let mut wanted = None;
    let mut kept: Vec<&str> = Vec::new();
    for pair in uri.query()?.split('&') {
        match pair.split_once('=') {
            Some((name, value)) if name == param => {
                if wanted.is_none() && !value.is_empty() {
                    wanted = Some(value);
                }
            }
            _ if pair.is_empty() => {}
            _ => kept.push(pair),
        }
    }
    let (target, _) = lookup(wanted?, locales)?;
    if target == current {
        return None;
    }
    let mut to = String::with_capacity(uri.path().len() + 8);
    to.push('/');
    to.push_str(target);
    if let Some(tail) = tail {
        to.push('/');
        to.push_str(tail);
    }
    if !kept.is_empty() {
        to.push('?');
        to.push_str(&kept.join("&"));
    }
    Some(to)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::path_for_query;
    use ::http::Uri;
    use alloc::string::String;
    use mf2_catalog::Dir;

    static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr), ("fr", Dir::Ltr), ("ar", Dir::Rtl)];

    fn to(uri: &str) -> Option<String> {
        path_for_query(&uri.parse::<Uri>().expect("a URI"), "lang", LOCALES)
    }

    #[test]
    fn a_query_that_disagrees_with_the_path_goes_to_its_url() {
        assert_eq!(to("/en/about?lang=fr").as_deref(), Some("/fr/about"));
        assert_eq!(
            to("/en/a/b?x=1&lang=ar&y=2").as_deref(),
            Some("/ar/a/b?x=1&y=2")
        );
        assert_eq!(to("/en/?lang=fr").as_deref(), Some("/fr/"));
        assert_eq!(to("/en?lang=fr").as_deref(), Some("/fr"));
        // The same matcher as negotiation: a region falls back to its language.
        assert_eq!(to("/en-GB/about?lang=fr-CA").as_deref(), Some("/fr/about"));
    }

    #[test]
    fn anything_else_passes_through() {
        // They agree.
        assert_eq!(to("/fr/about?lang=fr"), None);
        // No locale in the path: the query is negotiated as usual.
        assert_eq!(to("/about?lang=fr"), None);
        assert_eq!(to("/?lang=fr"), None);
        // No query, an empty one, or a locale the build does not have.
        assert_eq!(to("/en/about"), None);
        assert_eq!(to("/en/about?lang="), None);
        assert_eq!(to("/en/about?lang=de"), None);
    }
}
