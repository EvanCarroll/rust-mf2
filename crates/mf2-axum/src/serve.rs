//! `/i18n/*` — the catalogs, served from the server binary
//! (`plans/04-leptos-integration.md` §6 step 3).
//!
//! Two shapes of URL, and the difference between them is the whole caching
//! story:
//!
//! | URL | Answer | `Cache-Control` |
//! |---|---|---|
//! | `/i18n/en.3fa9c1.mf2b` — the content-hashed name the build wrote | the catalog | `public, max-age=31536000, immutable` |
//! | `/i18n/en` — a bare tag | `307` to the hashed URL | `no-cache` |
//!
//! The hashed URL is what the page's preload link carries, so a page load
//! never redirects. The bare tag exists only for a client that is switching
//! to a locale whose URL it does not know (§6, owner question 1), and it is
//! one round trip at switch time.
//!
//! **Precompression.** The variant is chosen from `Accept-Encoding` and
//! compressed **once per catalog**, on the first request that wants it, then
//! kept. Brotli is quality 11 with a 22-bit window, which is what P0.7
//! measured B7 against; gzip is level 9. Doing it at startup instead would
//! make a cold start pay for locales nobody asked for.

use std::collections::HashMap;
use std::io::Write as _;
use std::sync::{Arc, OnceLock, RwLock};

use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use http::header::{
    ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap,
    LOCATION, VARY,
};
use http::{HeaderValue, StatusCode};

/// Where the catalog routes are mounted. The client's `catalog_url` builds
/// the same path, so the two are one constant apart.
pub const CATALOG_PREFIX: &str = leptos_mf2::links::CATALOG_ROUTE;

/// A catalog is opaque bytes to everything but us; `octet-stream` is what
/// every proxy and browser handles without opinions.
const CATALOG_TYPE: HeaderValue = HeaderValue::from_static("application/octet-stream");
const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");
const NO_CACHE: HeaderValue = HeaderValue::from_static("no-cache");
const VARY_ENCODING: HeaderValue = HeaderValue::from_static("accept-encoding");

/// The encodings we can answer with, best first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    Brotli,
    Gzip,
    Identity,
}

impl Encoding {
    fn header(self) -> Option<HeaderValue> {
        match self {
            Encoding::Brotli => Some(HeaderValue::from_static("br")),
            Encoding::Gzip => Some(HeaderValue::from_static("gzip")),
            Encoding::Identity => None,
        }
    }
}

/// What a client will accept, best first, ignoring quality ordering beyond
/// "brotli if offered at all": a catalog is fetched once and cached for a
/// year, so the smallest encoding both sides know is always the right one.
fn best_encoding(headers: &HeaderMap) -> Encoding {
    let Some(accept) = headers.get(ACCEPT_ENCODING).and_then(|v| v.to_str().ok()) else {
        return Encoding::Identity;
    };
    let offers = |name: &str| {
        accept.split(',').any(|item| {
            let item = item.trim();
            let token = item.split(';').next().unwrap_or(item).trim();
            token.eq_ignore_ascii_case(name) && !item.contains("q=0,") && !item.ends_with("q=0")
        })
    };
    if offers("br") {
        Encoding::Brotli
    } else if offers("gzip") {
        Encoding::Gzip
    } else {
        Encoding::Identity
    }
}

/// One catalog's compressed forms, made on demand.
#[derive(Default)]
struct Variants {
    brotli: OnceLock<Option<Arc<[u8]>>>,
    gzip: OnceLock<Option<Arc<[u8]>>>,
}

fn variants() -> &'static RwLock<HashMap<&'static str, Arc<Variants>>> {
    static VARIANTS: OnceLock<RwLock<HashMap<&'static str, Arc<Variants>>>> = OnceLock::new();
    VARIANTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn variants_of(file: &'static str) -> Option<Arc<Variants>> {
    if let Ok(map) = variants().read()
        && let Some(found) = map.get(file)
    {
        return Some(Arc::clone(found));
    }
    let mut map = variants().write().ok()?;
    Some(Arc::clone(
        map.entry(file)
            .or_insert_with(|| Arc::new(Variants::default())),
    ))
}

/// Brotli at the quality P0.7 measured B7 with (11, window 22).
fn brotli(bytes: &[u8]) -> Option<Arc<[u8]>> {
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(bytes).ok()?;
    drop(writer);
    Some(Arc::from(out))
}

fn gzip(bytes: &[u8]) -> Option<Arc<[u8]>> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes).ok()?;
    encoder.finish().ok().map(Arc::from)
}

/// The catalog `file`, in `encoding` — falling back to the uncompressed
/// bytes if compression is unavailable for any reason.
fn body_for(
    file: &'static str,
    bytes: &'static [u8],
    encoding: Encoding,
) -> (Body, Encoding, usize) {
    let compressed = match encoding {
        Encoding::Identity => None,
        other => variants_of(file).and_then(|v| {
            let slot = match other {
                Encoding::Brotli => &v.brotli,
                _ => &v.gzip,
            };
            slot.get_or_init(|| match other {
                Encoding::Brotli => brotli(bytes),
                _ => gzip(bytes),
            })
            .clone()
        }),
    };
    match compressed {
        Some(data) => {
            let len = data.len();
            (Body::from(data.to_vec()), encoding, len)
        }
        None => (Body::from(bytes), Encoding::Identity, bytes.len()),
    }
}

/// `GET /i18n/{name}` — a hashed file name, or a bare locale tag.
async fn serve(Path(name): Path<String>, headers: HeaderMap) -> Response {
    // A hashed name: the immutable answer.
    if let Some((file, bytes)) = catalog_file(&name) {
        let encoding = best_encoding(&headers);
        let (body, encoding, length) = body_for(file, bytes, encoding);
        let mut response = Response::new(body);
        let out = response.headers_mut();
        out.insert(CONTENT_TYPE, CATALOG_TYPE);
        out.insert(CACHE_CONTROL, IMMUTABLE);
        out.insert(VARY, VARY_ENCODING);
        if let Ok(value) = HeaderValue::from_str(&length.to_string()) {
            out.insert(CONTENT_LENGTH, value);
        }
        if let Some(value) = encoding.header() {
            out.insert(CONTENT_ENCODING, value);
        }
        return response;
    }
    // A bare tag: redirect to the immutable URL. `307` rather than `301`
    // because the target changes with every deploy.
    if let Some(file) = leptos_mf2::catalog_name(&name) {
        let target = format!("{CATALOG_PREFIX}{file}");
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::TEMPORARY_REDIRECT;
        let out = response.headers_mut();
        if let Ok(value) = HeaderValue::from_str(&target) {
            out.insert(LOCATION, value);
        }
        out.insert(CACHE_CONTROL, NO_CACHE);
        return response;
    }
    (StatusCode::NOT_FOUND, "no such catalog").into_response()
}

/// The catalog published under `name`, if any — with the name itself as a
/// `&'static str`, which is the key the compressed variants are cached by.
fn catalog_file(name: &str) -> Option<(&'static str, &'static [u8])> {
    leptos_mf2::catalog_entries()
        .iter()
        .find(|entry| entry.file == name)
        .map(|entry| (entry.file, entry.bytes))
}

/// The routes that serve the catalogs. Mount them at the application's root:
///
/// ```ignore
/// let app = Router::new().merge(mf2_axum::catalog_routes()); // and the application's routes
/// ```
pub fn catalog_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route(&format!("{CATALOG_PREFIX}{{name}}"), get(serve))
}

#[cfg(test)]
mod tests {
    use super::{Encoding, best_encoding};
    use http::HeaderMap;
    use http::header::ACCEPT_ENCODING;

    fn accepting(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, value.parse().expect("a header value"));
        headers
    }

    #[test]
    fn brotli_wins_when_it_is_offered_at_all() {
        assert_eq!(
            best_encoding(&accepting("gzip, deflate, br")),
            Encoding::Brotli
        );
        assert_eq!(best_encoding(&accepting("gzip, deflate")), Encoding::Gzip);
        assert_eq!(best_encoding(&HeaderMap::new()), Encoding::Identity);
    }

    #[test]
    fn an_encoding_refused_with_q_0_is_not_used() {
        assert_eq!(best_encoding(&accepting("br;q=0, gzip")), Encoding::Gzip);
    }
}
