//! Server-only i18n plumbing (what `mf2-axum` would provide): the hand-built
//! catalogs, locale negotiation, the `additional_context` closure body and the
//! `/i18n/*` handler. Message text exists only in this module, which is not
//! compiled into the wasm.

use crate::msg::{IDS, MANIFEST_HASH};
use axum::{
    body::Body,
    extract::Path,
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_LANGUAGE, CONTENT_TYPE, COOKIE, LOCATION, VARY},
        request::Parts,
    },
    response::{IntoResponse, Response},
};
use leptos::prelude::use_context;
use leptos_axum::ResponseOptions;
use std::sync::Arc;
use tr::{Catalog, Dir, server::RequestI18n};

const EN: &[&str] = &[
    "mf2-two vertical slice",
    "Home",
    "Lazy route",
    "Streaming (out of order)",
    "Streaming (in order)",
    "Streaming (partially blocked)",
    "Async rendering",
    "Welcome! Every string on this page comes from a lazily loaded catalog.",
    "Language",
    "English",
    "العربية",
    "Search",
    "Type to search…",
    "This label arrived as a component prop.",
    "Toggle message",
    "Option A is showing.",
    "Option B is showing.",
    "Ask the server",
    "Lazy route",
    "This view's code was loaded from a separate WebAssembly chunk.",
    "Loaded on demand",
    "Streamed content",
    "This text was rendered on the server after the resource resolved.",
    "Loading…",
    "Page not found",
    "CANARY-QX7-TEXT-EN",
    "P0.10 text mismatch",
    "P0.10 structural mismatch",
    "Main",
    "This text came through Signal<String>.",
    "Hello from the server function.",
    "A second node in the same streamed chunk.",
];

const AR: &[&str] = &[
    "الشريحة العمودية لـ mf2-two",
    "الرئيسية",
    "المسار الكسول",
    "البث (خارج الترتيب)",
    "البث (بالترتيب)",
    "البث (محجوب جزئيًا)",
    "العرض غير المتزامن",
    "مرحبًا! كل نص في هذه الصفحة يأتي من فهرس يُحمَّل عند الحاجة.",
    "اللغة",
    "English",
    "العربية",
    "بحث",
    "اكتب للبحث…",
    "وصلت هذه التسمية كخاصية مكوّن.",
    "بدّل الرسالة",
    "الخيار أ معروض.",
    "الخيار ب معروض.",
    "اسأل الخادم",
    "المسار الكسول",
    "حُمِّلت شيفرة هذا العرض من جزء WebAssembly منفصل.",
    "حُمِّل عند الطلب",
    "محتوى مُبثّ",
    "عُرض هذا النص على الخادم بعد اكتمال تحميل المورد.",
    "جارٍ التحميل…",
    "الصفحة غير موجودة",
    "CANARY-QX7-TEXT-AR",
    "P0.10 اختلاف النص",
    "P0.10 اختلاف البنية",
    "رئيسي",
    "جاء هذا النص عبر Signal<String>.",
    "مرحبًا من دالة الخادم.",
    "عقدة ثانية في الجزء المُبثّ نفسه.",
];

pub const DEFAULT_LOCALE: &str = "en";
pub const COOKIE_NAME: &str = "mf2-locale";

/// One built catalog: bytes, content-hashed file name, and the per-request
/// state handed to `provide_context`.
pub struct Built {
    pub tag: &'static str,
    pub file: String,
    pub bytes: Arc<[u8]>,
    pub state: RequestI18n,
}

impl Built {
    fn new(tag: &'static str, dir: Dir, manifest: u64, messages: &[&str]) -> Self {
        assert_eq!(messages.len(), IDS.len(), "{tag}: message count != id count");
        let bytes: Arc<[u8]> = tr::catalog::write(tag, dir, manifest, messages).into();
        let file = format!("{tag}.{:016x}.mf2b", tr::fnv1a64(&bytes));
        // The server validates with the *real* manifest; the skewed catalog is
        // parsed with its own hash (it is only ever served, never used here).
        let catalog = Arc::new(Catalog::new(bytes.to_vec(), manifest).expect("valid catalog"));
        let state = RequestI18n { catalog, href: format!("/i18n/{file}").into() };
        Self { tag, file, bytes, state }
    }
}

pub struct Catalogs {
    pub locales: Vec<Built>,
    /// `en` compiled against a different manifest (deploy-skew test, `?skew=1`).
    pub skewed: Built,
}

impl Catalogs {
    pub fn build() -> Self {
        Self {
            locales: vec![
                Built::new("en", Dir::Ltr, MANIFEST_HASH, EN),
                Built::new("ar", Dir::Rtl, MANIFEST_HASH, AR),
            ],
            skewed: Built::new("en", Dir::Ltr, MANIFEST_HASH ^ 1, EN),
        }
    }

    pub fn default_state(&self) -> RequestI18n {
        self.by_tag(DEFAULT_LOCALE).expect("default locale").state.clone()
    }

    pub fn by_tag(&self, tag: &str) -> Option<&Built> {
        self.locales.iter().find(|b| b.tag == tag)
    }

    pub fn by_file(&self, file: &str) -> Option<&Built> {
        self.locales.iter().chain(std::iter::once(&self.skewed)).find(|b| b.file == file)
    }

    /// cookie → `Accept-Language` → default. (mf2-axum makes this a trait.)
    pub fn negotiate(&self, parts: Option<&Parts>) -> &Built {
        let Some(parts) = parts else {
            return self.by_tag(DEFAULT_LOCALE).expect("default locale");
        };
        let from_cookie = parts
            .headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|kv| kv.trim().split_once('='))
            .find(|(k, _)| *k == COOKIE_NAME)
            .and_then(|(_, v)| self.by_tag(v.trim()));
        if let Some(b) = from_cookie {
            return b;
        }
        let from_header = parts
            .headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .into_iter()
            .flat_map(|v| v.split(','))
            .filter_map(|item| item.split(';').next())
            .filter_map(|tag| tag.trim().split('-').next())
            .find_map(|primary| self.by_tag(&primary.to_ascii_lowercase()));
        from_header.unwrap_or_else(|| self.by_tag(DEFAULT_LOCALE).expect("default locale"))
    }
}

/// Body of the `additional_context` closure given to all leptos_axum
/// `*_with_context` entry points. With no request parts (route-list
/// generation uses mock parts; static files have none) it provides the default.
pub fn provide_i18n(catalogs: &Catalogs) {
    let parts = use_context::<Parts>();
    let skew = parts
        .as_ref()
        .and_then(|p| p.uri.query())
        .is_some_and(|q| q.split('&').any(|kv| kv == "skew=1"));
    let built = if skew { &catalogs.skewed } else { catalogs.negotiate(parts.as_ref()) };
    tr::server::provide(built.state.clone());
    if let Some(res) = use_context::<ResponseOptions>() {
        res.insert_header(CONTENT_LANGUAGE, HeaderValue::from_static(built.tag));
        res.insert_header(VARY, HeaderValue::from_static("Cookie, Accept-Language"));
    }
}

/// `GET /i18n/{file}`: `<tag>.<hash>.mf2b` → immutable bytes; `<tag>` → a
/// short-lived redirect to the hashed URL (used by `set_locale`).
pub fn serve_catalog(catalogs: &Catalogs, Path(file): Path<String>) -> Response {
    if let Some(b) = catalogs.by_file(&file) {
        return Response::builder()
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(Body::from(b.bytes.to_vec()))
            .map_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response(), IntoResponse::into_response);
    }
    if let Some(b) = catalogs.by_tag(&file) {
        return Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(LOCATION, format!("/i18n/{}", b.file))
            .header(CACHE_CONTROL, "no-cache")
            .body(Body::empty())
            .map_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response(), IntoResponse::into_response);
    }
    StatusCode::NOT_FOUND.into_response()
}
