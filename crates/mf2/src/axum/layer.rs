//! [`Negotiator`] as a tower layer (`plans/04-leptos-integration.md` §12.5).
//!
//! The layer negotiates each request once and puts the answer in the
//! request's extensions. A Leptos render finds it through the request
//! `Parts` that `leptos_axum` provides in the context, and the generated
//! `Locale` extractor through its own `Parts`, so no entry point needs to be
//! told about it. The response headers go on only when something read the
//! answer: a static file or a catalog served under the layer depends on no
//! language, and says none.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU8, Ordering};
use core::task::{Context, Poll};

use ::http::header::{CONTENT_LANGUAGE, HeaderMap, HeaderName, HeaderValue, VARY};
use ::http::{Request, Response};
use tower_layer::Layer;
use tower_service::Service;

use super::negotiate::{Negotiated, Negotiator};

/// Something read the answer: the response depends on the language.
const READ: u8 = 1;
/// A render applied the reader's time zone from the `mf2_tz` cookie.
const ZONED: u8 = 2;

/// What the layer negotiated, in the request's extensions.
#[derive(Clone, Debug)]
pub(crate) struct RequestLocale {
    negotiated: Negotiated,
    query: Option<&'static str>,
    reads: Arc<AtomicU8>,
}

impl RequestLocale {
    /// The answer; the response now depends on it.
    pub(crate) fn read(&self) -> &Negotiated {
        self.reads.fetch_or(READ, Ordering::Relaxed);
        &self.negotiated
    }

    /// The installed query source's name: what the switcher's form submits,
    /// and what `path_prefix_redirect` reads.
    pub(crate) fn query(&self) -> Option<&'static str> {
        self.query
    }

    /// The render used the reader's time zone: the response also depends on
    /// `Cookie`.
    #[cfg_attr(
        not(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))),
        allow(dead_code, reason = "only a Leptos render reads it")
    )]
    pub(crate) fn zoned(&self) {
        self.reads.fetch_or(READ | ZONED, Ordering::Relaxed);
    }
}

impl<S> Layer<S> for Negotiator {
    type Service = Negotiate<S>;

    fn layer(&self, inner: S) -> Negotiate<S> {
        Negotiate {
            inner,
            vary: self.vary(),
            negotiator: Arc::new(self.clone()),
        }
    }
}

/// The service [`Negotiator`] wraps a router's in.
#[derive(Clone, Debug)]
pub struct Negotiate<S> {
    inner: S,
    negotiator: Arc<Negotiator>,
    vary: Option<HeaderValue>,
}

impl<S, B, R> Service<Request<B>> for Negotiate<S>
where
    S: Service<Request<B>, Response = Response<R>>,
    S::Future: Send + 'static,
    S::Error: 'static,
    R: 'static,
{
    type Response = Response<R>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<R>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let (mut parts, body) = request.into_parts();
        let negotiated = self.negotiator.negotiate(&parts);
        let tag = negotiated.tag;
        let sinks: Vec<(HeaderName, HeaderValue)> = self.negotiator.store(&negotiated).collect();
        let reads = Arc::new(AtomicU8::new(0));
        parts.extensions.insert(RequestLocale {
            negotiated,
            query: self.negotiator.query_name(),
            reads: Arc::clone(&reads),
        });
        let vary = self.vary.clone();
        let future = self.inner.call(Request::from_parts(parts, body));
        Box::pin(async move {
            let mut response = future.await?;
            let reads = reads.load(Ordering::Relaxed);
            if reads & READ != 0 {
                answer(response.headers_mut(), tag, vary, sinks, reads & ZONED != 0);
            }
            Ok(response)
        })
    }
}

/// `Content-Language`, `Vary` (with `cookie` when the page is in the
/// reader's zone and the sources did not already name it) and every sink's
/// header.
fn answer(
    headers: &mut HeaderMap,
    tag: &'static str,
    vary: Option<HeaderValue>,
    sinks: Vec<(HeaderName, HeaderValue)>,
    zoned: bool,
) {
    if let Ok(value) = HeaderValue::from_str(tag) {
        headers.insert(CONTENT_LANGUAGE, value);
    }
    let cookie_named = vary
        .as_ref()
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|name| name.trim().eq_ignore_ascii_case("cookie"))
        });
    if let Some(vary) = vary {
        headers.append(VARY, vary);
    }
    if zoned && !cookie_named {
        headers.append(VARY, HeaderValue::from_static("cookie"));
    }
    for (name, value) in sinks {
        headers.append(name, value);
    }
}
