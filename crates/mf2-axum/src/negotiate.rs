//! Negotiation: an **ordered list of typed sources and sinks**
//! (`plans/04-leptos-integration.md` §6, §11 item 5).
//!
//! Not a boolean matrix. The prior-art audit found ~60 hand-parsed
//! parameters covering the full `initial_language_from_<source>_to_<target>`
//! cross-product, and a live copy-paste bug inside it. Here a source is a
//! trait, a sink is a trait, and configuration is the order they are listed
//! in — so adding one is one implementation, not 2ⁿ new parameter names.
//!
//! The first source that yields a locale this build has **wins**; the
//! negotiated tag is then written to every sink, and serialized into the
//! page, so the client never negotiates again (§11 item 3).

use std::borrow::Cow;

use http::header::{ACCEPT_LANGUAGE, COOKIE, HeaderName, HeaderValue, SET_COOKIE};
use http::request::Parts;
use mf2_catalog::Dir;

/// What negotiation decided, for one request.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct Negotiated {
    /// The tag, always one this build has.
    pub tag: &'static str,
    /// Its base direction, as the build recorded it.
    pub dir: Dir,
    /// Which source matched, or `"default"`.
    pub from: &'static str,
    /// Whether a source matched at all. `false` means the default locale was
    /// used, which is a normal outcome, not an error.
    pub matched: bool,
}

impl Negotiated {
    /// `<html dir>`.
    #[must_use]
    pub fn dir_attr(&self) -> &'static str {
        if self.dir == Dir::Rtl { "rtl" } else { "ltr" }
    }
}

/// Somewhere a request can carry a locale.
pub trait LocaleSource: Send + Sync + std::fmt::Debug {
    /// A short name, which is what [`Negotiated::from`] reports.
    fn name(&self) -> &'static str;

    /// The request header this source reads, if any. Every one of these goes
    /// into `Vary`, because the response body depends on it.
    fn vary(&self) -> Option<HeaderName> {
        None
    }

    /// The tags this request offers, best first. Called once per request.
    fn candidates<'r>(&self, parts: &'r Parts, out: &mut Vec<Cow<'r, str>>);
}

/// Somewhere the negotiated locale is written back to.
pub trait LocaleSink: Send + Sync + std::fmt::Debug {
    /// A short name, for diagnostics.
    fn name(&self) -> &'static str;

    /// The response header this sink adds, if any.
    fn store(&self, negotiated: &Negotiated) -> Option<(HeaderName, HeaderValue)>;
}

/// A cookie, read and written: the only source that remembers a choice the
/// user made, so it comes first by default.
///
/// As a sink it writes only an **explicit** choice — a locale that came from
/// the query (`?lang=`) or the path. A locale guessed from `Accept-Language`
/// or the default is not remembered, and a cookie that was read is not
/// written back, so its expiry does not slide. The client writes the cookie
/// itself on every switch.
#[derive(Clone, Debug)]
pub struct CookieLocale {
    /// The cookie's name.
    pub name: &'static str,
    /// `Max-Age`, in seconds. A year by default.
    pub max_age: u32,
    /// `Path`.
    pub path: &'static str,
    /// `SameSite`. `Lax` by default: a locale is not a credential, and `Lax`
    /// survives a link from another site.
    pub same_site: &'static str,
    /// `Secure`.
    pub secure: bool,
}

impl Default for CookieLocale {
    fn default() -> CookieLocale {
        CookieLocale {
            // The name a `static-locale` client writes on a switch.
            name: leptos_mf2::links::LOCALE_COOKIE,
            max_age: 31_536_000,
            path: "/",
            same_site: "Lax",
            secure: true,
        }
    }
}

impl LocaleSource for CookieLocale {
    fn name(&self) -> &'static str {
        "cookie"
    }

    fn vary(&self) -> Option<HeaderName> {
        Some(COOKIE)
    }

    fn candidates<'r>(&self, parts: &'r Parts, out: &mut Vec<Cow<'r, str>>) {
        if let Some(value) = cookie(parts, self.name) {
            out.push(Cow::Borrowed(value));
        }
    }
}

/// The value of the first cookie named `name` in the request, trimmed.
pub(crate) fn cookie<'r>(parts: &'r Parts, name: &str) -> Option<&'r str> {
    let header = parts.headers.get(COOKIE).and_then(|v| v.to_str().ok())?;
    header.split(';').find_map(|pair| {
        let (n, value) = pair.trim_start().split_once('=')?;
        (n.trim() == name).then(|| value.trim())
    })
}

impl LocaleSink for CookieLocale {
    fn name(&self) -> &'static str {
        "cookie"
    }

    fn store(&self, negotiated: &Negotiated) -> Option<(HeaderName, HeaderValue)> {
        if !matches!(negotiated.from, "query" | "path") {
            return None;
        }
        let mut cookie = format!(
            "{}={}; Max-Age={}; Path={}; SameSite={}",
            self.name, negotiated.tag, self.max_age, self.path, self.same_site
        );
        if self.secure {
            cookie.push_str("; Secure");
        }
        HeaderValue::from_str(&cookie)
            .ok()
            .map(|value| (SET_COOKIE, value))
    }
}

/// `Accept-Language`, in quality order.
#[derive(Clone, Copy, Debug, Default)]
pub struct AcceptLanguage;

impl LocaleSource for AcceptLanguage {
    fn name(&self) -> &'static str {
        "accept-language"
    }

    fn vary(&self) -> Option<HeaderName> {
        Some(ACCEPT_LANGUAGE)
    }

    fn candidates<'r>(&self, parts: &'r Parts, out: &mut Vec<Cow<'r, str>>) {
        let Some(header) = parts
            .headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
        else {
            return;
        };
        // (quality ×1000, tag), stably sorted so that equal qualities keep
        // the order the client wrote.
        let mut ranked: Vec<(u32, &str)> = Vec::new();
        for item in header.split(',') {
            let mut fields = item.split(';');
            let Some(tag) = fields.next().map(str::trim) else {
                continue;
            };
            if tag.is_empty() {
                continue;
            }
            let quality = fields
                .find_map(|f| f.trim().strip_prefix("q=").map(quality_milli))
                .unwrap_or(1000);
            if quality > 0 {
                ranked.push((quality, tag));
            }
        }
        ranked.sort_by_key(|(quality, _)| core::cmp::Reverse(*quality));
        out.extend(ranked.into_iter().map(|(_, tag)| Cow::Borrowed(tag)));
    }
}

/// `q=0.8` as thousandths, saturating; anything unparseable is `q=1`, which
/// is what a lenient parser should do with a header it did not write.
fn quality_milli(text: &str) -> u32 {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let whole: u32 = whole.parse().unwrap_or(1);
    let mut milli = whole.saturating_mul(1000);
    for (i, digit) in fraction.chars().take(3).enumerate() {
        let Some(d) = digit.to_digit(10) else { break };
        let scale = match i {
            0 => 100,
            1 => 10,
            _ => 1,
        };
        milli += d * scale;
    }
    milli.min(1000)
}

/// A path prefix — `/es/…` — for sites that want crawlable per-locale URLs.
#[derive(Clone, Copy, Debug, Default)]
pub struct PathPrefix;

impl LocaleSource for PathPrefix {
    fn name(&self) -> &'static str {
        "path"
    }

    fn candidates<'r>(&self, parts: &'r Parts, out: &mut Vec<Cow<'r, str>>) {
        let path = parts.uri.path();
        let first = path.trim_start_matches('/').split('/').next();
        if let Some(segment) = first
            && !segment.is_empty()
        {
            out.push(Cow::Borrowed(segment));
        }
    }
}

/// A query parameter — `?lang=es` — which is how a link can force a locale
/// without a cookie.
#[derive(Clone, Copy, Debug)]
pub struct QueryParam(pub &'static str);

impl Default for QueryParam {
    fn default() -> QueryParam {
        // The name a `static-locale` client removes on a switch.
        QueryParam(leptos_mf2::links::LOCALE_QUERY)
    }
}

impl LocaleSource for QueryParam {
    fn name(&self) -> &'static str {
        "query"
    }

    fn candidates<'r>(&self, parts: &'r Parts, out: &mut Vec<Cow<'r, str>>) {
        let Some(query) = parts.uri.query() else {
            return;
        };
        for pair in query.split('&') {
            if let Some((name, value)) = pair.split_once('=')
                && name == self.0
                && !value.is_empty()
            {
                out.push(Cow::Borrowed(value));
                return;
            }
        }
    }
}

/// The ordered list itself.
#[derive(Debug)]
pub struct Negotiator {
    sources: Vec<Box<dyn LocaleSource>>,
    sinks: Vec<Box<dyn LocaleSink>>,
    locales: &'static [(&'static str, Dir)],
    default: &'static str,
    vary: Option<HeaderValue>,
}

impl Negotiator {
    /// An empty negotiator over the build's own locales: every request gets
    /// the source locale until a source is added.
    ///
    /// The locales come from [`install`](crate::install), so call this after
    /// it.
    #[must_use]
    pub fn empty() -> Negotiator {
        Negotiator::over(leptos_mf2::locales(), leptos_mf2::source_locale())
    }

    /// An empty negotiator over an explicit locale table — what a test uses,
    /// and what an application uses when it offers fewer locales than it
    /// built.
    #[must_use]
    pub fn over(locales: &'static [(&'static str, Dir)], default: &'static str) -> Negotiator {
        Negotiator {
            sources: Vec::new(),
            sinks: Vec::new(),
            locales,
            default,
            vary: None,
        }
    }

    /// Appends a source. Order is precedence.
    #[must_use]
    pub fn source(mut self, source: impl LocaleSource + 'static) -> Negotiator {
        self.sources.push(Box::new(source));
        self.vary = None;
        self
    }

    /// Appends a sink.
    #[must_use]
    pub fn sink(mut self, sink: impl LocaleSink + 'static) -> Negotiator {
        self.sinks.push(Box::new(sink));
        self
    }

    /// Overrides the locale a request with no match gets. It must be one the
    /// build has, or the source locale is kept.
    #[must_use]
    pub fn default_locale(mut self, tag: &str) -> Negotiator {
        if let Some((found, _)) = self.locales.iter().find(|(t, _)| *t == tag) {
            self.default = found;
        }
        self
    }

    /// Every locale this build has, with its direction.
    #[must_use]
    pub fn locales(&self) -> &'static [(&'static str, Dir)] {
        self.locales
    }

    /// What this request should be answered in.
    #[must_use]
    pub fn negotiate(&self, parts: &Parts) -> Negotiated {
        let mut candidates: Vec<Cow<'_, str>> = Vec::new();
        for source in &self.sources {
            candidates.clear();
            source.candidates(parts, &mut candidates);
            // A source's candidates are one reader's list, best first: the
            // one matcher weighs them together (a later entry demoted), as
            // UTS #35 Part 1 matches a list.
            if let Some((tag, dir)) =
                leptos_mf2::best_locale(candidates.iter().map(AsRef::as_ref), self.locales)
            {
                return Negotiated {
                    tag,
                    dir,
                    from: source.name(),
                    matched: true,
                };
            }
        }
        Negotiated {
            tag: self.default,
            dir: self.dir_of(self.default),
            from: "default",
            matched: false,
        }
    }

    fn dir_of(&self, tag: &str) -> Dir {
        self.locales
            .iter()
            .find(|(t, _)| *t == tag)
            .map_or(Dir::Ltr, |(_, d)| *d)
    }

    /// The `Vary` value: every request header a source reads. A response
    /// that depends on `Cookie` and is cached without saying so is the
    /// classic way to serve one user's language to another.
    #[must_use]
    pub fn vary(&self) -> Option<HeaderValue> {
        if let Some(vary) = &self.vary {
            return Some(vary.clone());
        }
        let mut names: Vec<String> = Vec::new();
        for source in &self.sources {
            if let Some(name) = source.vary()
                && !names.iter().any(|n| n.eq_ignore_ascii_case(name.as_str()))
            {
                // `HeaderName` is always lowercase ASCII, so this is already
                // the canonical spelling.
                names.push(name.as_str().to_owned());
            }
        }
        if names.is_empty() {
            return None;
        }
        HeaderValue::from_str(&names.join(", ")).ok()
    }

    /// The headers every sink wants on this response.
    pub fn store(
        &self,
        negotiated: &Negotiated,
    ) -> impl Iterator<Item = (HeaderName, HeaderValue)> {
        self.sinks
            .iter()
            .filter_map(move |sink| sink.store(negotiated))
    }
}

impl Default for Negotiator {
    /// Cookie, then `Accept-Language`, with the cookie as the sink: what a
    /// site wants unless it has said otherwise. Neither source is an explicit
    /// choice, so the sink writes nothing until a query or path source is
    /// added; the client writes the cookie on a switch. A path prefix is
    /// deliberately not here — it changes URLs, so a site opts into it.
    fn default() -> Negotiator {
        Negotiator::empty()
            .source(CookieLocale::default())
            .source(AcceptLanguage)
            .sink(CookieLocale::default())
    }
}

/// The one matcher (plans/19-native-and-terminal.md §9), shared with a
/// client-only application's boot: the locale that best serves one tag.
pub(crate) fn lookup(
    candidate: &str,
    locales: &[(&'static str, Dir)],
) -> Option<(&'static str, Dir)> {
    leptos_mf2::lookup_locale(candidate, locales)
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptLanguage, CookieLocale, Dir, LocaleSink, LocaleSource, Negotiated, Negotiator,
        lookup, quality_milli,
    };
    use http::Request;
    use std::borrow::Cow;

    static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr), ("fr-CA", Dir::Ltr), ("ar", Dir::Rtl)];

    fn negotiator() -> Negotiator {
        Negotiator::over(LOCALES, "en")
    }

    fn parts(headers: &[(&str, &str)], uri: &str) -> http::request::Parts {
        let mut builder = Request::builder().uri(uri);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(()).expect("a request").into_parts().0
    }

    #[test]
    fn lookup_finds_the_closest_language_by_cldr() {
        assert_eq!(lookup("en", LOCALES).map(|l| l.0), Some("en"));
        // Another region of English: 5 (`en-*-*`).
        assert_eq!(lookup("EN-GB", LOCALES).map(|l| l.0), Some("en"));
        // `fr` is not in the list, but `fr-CA` is: 4 (`*-*-*`).
        assert_eq!(lookup("fr", LOCALES).map(|l| l.0), Some("fr-CA"));
        assert_eq!(lookup("de", LOCALES), None);
        assert_eq!(lookup("*", LOCALES), None);
    }

    #[test]
    fn a_source_list_is_matched_as_one_list() {
        let negotiator = Negotiator::over(
            &[("de", Dir::Ltr), ("fr", Dir::Ltr), ("zh", Dir::Ltr)],
            "de",
        )
        .source(AcceptLanguage);
        let pick = |header: &str| {
            negotiator
                .negotiate(&parts(&[("accept-language", header)], "/"))
                .tag
        };
        // UTS #35 Part 1's demotion example: a regional variant of the first
        // language (4) beats an exact second one (5).
        assert_eq!(pick("de-AT, fr;q=0.9"), "de");
        // Traditional Chinese is not served Simplified (question 15), but
        // the list's plain `zh` is: 5.
        assert_eq!(pick("zh-TW, zh;q=0.9"), "zh");
        assert_eq!(pick("zh-TW, fr;q=0.5"), "fr");
    }

    #[test]
    fn accept_language_is_read_in_quality_order() {
        let parts = parts(&[("accept-language", "de;q=0.9, fr;q=0.95, en;q=0.2")], "/");
        let mut out: Vec<Cow<'_, str>> = Vec::new();
        AcceptLanguage.candidates(&parts, &mut out);
        assert_eq!(out, ["fr", "de", "en"]);
    }

    #[test]
    fn quality_parses_thousandths_and_clamps() {
        assert_eq!(quality_milli("1"), 1000);
        assert_eq!(quality_milli("0.5"), 500);
        assert_eq!(quality_milli("0.333"), 333);
        assert_eq!(quality_milli("0.3339"), 333);
        assert_eq!(quality_milli("7"), 1000);
    }

    #[test]
    fn the_first_source_that_matches_wins() {
        let negotiator = negotiator()
            .source(super::CookieLocale::default())
            .source(AcceptLanguage);
        // The cookie is listed first, so it beats a better `Accept-Language`.
        let parts = parts(
            &[
                ("cookie", "theme=dark; mf2_locale=ar"),
                ("accept-language", "en"),
            ],
            "/",
        );
        let negotiated = negotiator.negotiate(&parts);
        assert_eq!(negotiated.tag, "ar");
        assert_eq!(negotiated.from, "cookie");
        assert_eq!(negotiated.dir_attr(), "rtl");
    }

    #[test]
    fn a_request_with_nothing_to_go_on_gets_the_default_and_says_so() {
        let negotiator = negotiator().source(AcceptLanguage);
        let negotiated = negotiator.negotiate(&parts(&[], "/"));
        assert!(!negotiated.matched);
        assert_eq!(negotiated.from, "default");
    }

    #[test]
    fn vary_names_every_header_a_source_reads() {
        let negotiator = negotiator()
            .source(super::CookieLocale::default())
            .source(AcceptLanguage)
            .source(super::PathPrefix);
        let vary = negotiator.vary().expect("two headers are read");
        assert_eq!(vary.to_str().unwrap_or(""), "cookie, accept-language");
    }

    #[test]
    fn a_path_prefix_is_read_from_the_first_segment() {
        let negotiator = negotiator().source(super::PathPrefix);
        assert_eq!(negotiator.negotiate(&parts(&[], "/ar/inbox")).tag, "ar");
        assert!(!negotiator.negotiate(&parts(&[], "/inbox")).matched);
    }

    fn stored_from(from: &'static str) -> Option<String> {
        let negotiated = Negotiated {
            tag: "fr-CA",
            dir: Dir::Ltr,
            from,
            matched: from != "default",
        };
        CookieLocale::default()
            .store(&negotiated)
            .map(|(_, value)| value.to_str().unwrap_or("").to_owned())
    }

    #[test]
    fn the_cookie_is_written_for_an_explicit_choice() {
        for from in ["query", "path"] {
            assert_eq!(
                stored_from(from).as_deref(),
                Some("mf2_locale=fr-CA; Max-Age=31536000; Path=/; SameSite=Lax; Secure"),
                "from {from}"
            );
        }
    }

    #[test]
    fn the_cookie_is_not_written_for_a_guess_or_a_cookie_already_there() {
        for from in ["cookie", "accept-language", "default"] {
            assert_eq!(stored_from(from), None, "from {from}");
        }
    }
}
