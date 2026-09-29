//! The page's own i18n furniture: [`html_lang`], and the six components —
//! the preload link, the catalog map, the islands gate, the `hreflang`
//! block, the switcher and its options.
//!
//! The six are the active Leptos line's helper crate's (`mf2-leptos-ui-0-9`
//! or `mf2-leptos-ui-0-8`): Leptos's `view!` and `#[component]` write
//! `::leptos` into the crate that uses them, and this crate reaches its two
//! lines under names of its own. The helpers cannot depend on this crate,
//! so each component is generic over their `Layer` trait — what it reads
//! from here — and [`Mf2`] implements it. Each function below is the
//! helper's component with [`Mf2`] chosen: it takes the helper's props, so
//! `view!` builds it exactly as it builds any component, and the calls into
//! this crate are resolved when it is compiled. Nothing is installed at
//! start-up, and an application that renders none of the six links none of
//! them.
//!
//! WCAG 2.2 AA is a requirement here, not a nicety, and two of these
//! components exist because getting them right by hand is fiddly:
//!
//! * `<html lang dir>` must be correct and must **update on a switch**
//!   (3.1.1). `set_document_lang` (a client function: `hydrate` or `csr`)
//!   does the update; [`html_lang`] gives the shell the pair to render.
//! * A language control must be a real labelled control, and each language
//!   must be named **in its own language, with its own `lang`** — otherwise
//!   a screen reader pronounces "Français" with an English voice.
//!
//! **Where the option text comes from, and why it is not in the wasm.**
//! [`LocaleOption`] takes its text as children, so each language's name is a
//! message of the application's own catalog — `language.fr` in *every*
//! locale's catalog — and never a literal in the client. What keeps the
//! autonyms out of the wasm is that they are catalog data.

use alloc::string::String;
use alloc::vec::Vec;

use mf2_catalog::Dir;

use crate::line::leptos::IntoView;
use crate::line::ui;

/// The layer the six components read: this crate. What each component's
/// props are generic over; an application never names it.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct Mf2;

impl ui::Layer for Mf2 {
    const LOCALE_QUERY: &'static str = crate::leptos::links::LOCALE_QUERY;
    const ISLANDS_GATE: &'static str = crate::leptos::links::ISLANDS_GATE;
    const CATALOG_LINK_REL: &'static str = crate::leptos::links::CATALOG_LINK_REL;

    fn locales() -> &'static [(&'static str, Dir)] {
        crate::leptos::state::locales()
    }

    fn source_locale() -> &'static str {
        crate::leptos::state::source_locale()
    }

    fn html_lang() -> (String, &'static str) {
        html_lang()
    }

    #[cfg(feature = "ssr")]
    fn preload() -> Option<(String, Option<String>)> {
        let href = crate::leptos::catalog::active()
            .and_then(|catalog| crate::leptos::catalog::catalog_name(catalog.locale()))
            .map(crate::leptos::links::catalog_href)?;
        let zone = crate::leptos::catalog::request_time_zone()
            .and_then(|zone| crate::leptos::zone::zone_name(&zone).map(String::from));
        Some((href, zone))
    }

    #[cfg(not(feature = "ssr"))]
    fn preload() -> Option<(String, Option<String>)> {
        None
    }

    #[cfg(feature = "ssr")]
    fn catalog_links() -> Vec<(&'static str, String)> {
        crate::leptos::catalog::catalog_entries()
            .iter()
            .map(|entry| (entry.tag, crate::leptos::links::catalog_href(entry.file)))
            .collect()
    }

    #[cfg(not(feature = "ssr"))]
    fn catalog_links() -> Vec<(&'static str, String)> {
        Vec::new()
    }

    #[cfg(any(feature = "hydrate", feature = "csr"))]
    fn switch(tag: String) {
        crate::line::leptos::task::spawn_local(async move {
            if let Err(error) = crate::leptos::set_locale(&tag).await {
                web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(match error {
                    crate::error::LoadError::UnknownLocale => "mf2: no such locale",
                    _ => "mf2: the locale could not be switched; the page is unchanged",
                }));
            }
        });
    }

    #[cfg(feature = "ssr")]
    fn switch(_tag: String) {}
}

/// The props of [`CatalogPreload`] (none).
pub type CatalogPreloadProps = ui::CatalogPreloadProps<Mf2>;
/// The props of [`CatalogLinks`] (none).
pub type CatalogLinksProps = ui::CatalogLinksProps<Mf2>;
/// The props of [`IslandsGate`] (none).
pub type IslandsGateProps = ui::IslandsGateProps<Mf2>;
/// The props of [`AlternateLinks`].
pub type AlternateLinksProps = ui::AlternateLinksProps<Mf2>;
/// The props of [`LocaleSwitcher`].
pub type LocaleSwitcherProps = ui::LocaleSwitcherProps<Mf2>;
/// The props of [`LocaleOption`].
pub type LocaleOptionProps = ui::LocaleOptionProps<Mf2>;

/// The `lang` and `dir` the shell should put on `<html>`: the catalog's own
/// locale, so the page always says what it is in (WCAG 3.1.1).
///
/// With no catalog — before boot, or a build with none — it is the source
/// locale and `ltr`.
#[must_use]
pub fn html_lang() -> (String, &'static str) {
    match crate::leptos::catalog::active() {
        Some(catalog) => (
            catalog.locale().into(),
            if catalog.dir() == Dir::Rtl {
                "rtl"
            } else {
                "ltr"
            },
        ),
        None => (crate::leptos::state::source_locale().into(), "ltr"),
    }
}

/// The preload link for the catalog of the locale this page is being
/// rendered in — the boot data, and the reason the client needs no inline
/// script and makes no extra request.
///
/// It renders only on the server: the client reads this link, it does not
/// write it, and the shell's `<head>` is not hydrated.
///
/// When the request was rendered in the reader's time zone, the link says
/// which (`data-mf2-zone`), so that the client knows whether its dates need
/// correcting. Absent, the page was rendered in [`Setup`]'s zone.
///
/// [`Setup`]: crate::leptos::Setup
#[must_use]
#[allow(non_snake_case)]
pub fn CatalogPreload() -> impl IntoView {
    ui::CatalogPreload(CatalogPreloadProps::builder().build())
}

/// The in-page tag → URL map: one `<link>` per locale, so that a switch needs
/// no round trip to learn the hashed URL. Server only, like
/// [`CatalogPreload`].
///
/// A site that would rather keep its pages a few bytes smaller simply does
/// not render this, and the switch redirects through `/i18n/<tag>` instead.
///
/// **Every** locale, including the one the page was rendered in: after one
/// switch the page's locale is no longer the one the reader may want back.
#[must_use]
#[allow(non_snake_case)]
pub fn CatalogLinks() -> impl IntoView {
    ui::CatalogLinks(CatalogLinksProps::builder().build())
}

/// The first thing in an islands page's `<body>`: an empty island that
/// Leptos' island walk awaits until the catalog is installed, so that every
/// island after it hydrates against the catalog the page was rendered with
/// (`hydrate_islands`, with `hydrate`, says why nothing else can wait).
/// Pair it with [`islands_gate!`](crate::leptos::islands_gate) in the
/// client.
///
/// It must come before every island in document order, and outside all of
/// them. It has no content and no role, so assistive technology never meets
/// it.
#[must_use]
#[allow(non_snake_case)]
pub fn IslandsGate() -> impl IntoView {
    ui::IslandsGate(IslandsGateProps::builder().build())
}

/// `<link rel="alternate" hreflang>` for a site whose locales have their own
/// URLs (a path prefix).
///
/// Its prop, `href_of: fn(&str) -> String`, maps a tag to that locale's URL
/// for the page being rendered. `x-default` points at the source locale,
/// which is what a crawler uses when it has no better match.
#[must_use]
#[allow(non_snake_case)]
pub fn AlternateLinks(props: AlternateLinksProps) -> impl IntoView {
    ui::AlternateLinks(props)
}

/// A labelled native control that switches locale, applied by a button.
///
/// A `<form method="get">` holding a `<select name="lang">` inside its
/// `<label>` and a submit button. Choosing a language changes nothing until
/// the button is pressed: the keyboard fires a `<select>`'s `change` on
/// every arrow key, so switching on it would change the page's language —
/// or, under `static-locale`, reload it — per keypress (WCAG 3.2.2, failure
/// F37).
///
/// With no client code — before the wasm loads, after a failed boot, or on
/// an islands page where the switcher is not an island — the form's own
/// `GET ?lang=…` is the switch, which `mf2-axum`'s `QueryParam` negotiates.
/// Under `hydrate` and `csr` the submit is intercepted and becomes
/// `set_locale`, live, with focus left on the button.
///
/// The `<select>` is inside its `<label>`, so there is no fixed `id` and a
/// page may carry two switchers (a header and a footer). The options are
/// [`LocaleOption`]s the caller writes, each carrying its own `lang`.
///
/// Its props:
/// * `label` (into `TextProp`) — the control's accessible name: a `<label>`,
///   not a `placeholder` and not an `aria-label` on its own, so that it is
///   visible as well as announced (WCAG 3.3.2);
/// * `button` (into `TextProp`) — the submit button's text, the
///   application's own message in the page's language;
/// * `children` — the options: one [`LocaleOption`] per locale offered;
/// * `href_of` (optional, `fn(&str) -> String`) — for a site whose
///   languages live in its URLs (`/fr/…`, `mf2-axum`'s `PathPrefix`), the
///   URL of the current page in the given locale. Each option then carries
///   it as `data-mf2-href`, and the submit navigates there instead of
///   switching in place — a `?lang=` cannot outrank the path. Without the
///   wasm the form's `?lang=` still goes to the server, and
///   `mf2_axum::path_prefix_redirect` sends it on to that URL.
///
/// ```ignore
/// <LocaleSwitcher label=tr!("choose-language") button=tr!("apply-language")>
///     <LocaleOption tag="en">{tr!("language-en")}</LocaleOption>
///     <LocaleOption tag="fr">{tr!("language-fr")}</LocaleOption>
/// </LocaleSwitcher>
/// ```
#[must_use]
#[allow(non_snake_case)]
pub fn LocaleSwitcher(props: LocaleSwitcherProps) -> impl IntoView {
    ui::LocaleSwitcher(props)
}

/// One `<option>`, named in its own language and marked as being in it.
///
/// Its props: `tag`, the BCP 47 tag this option selects (a
/// `&'static str`), and `children`, the language's name **in that
/// language** — from the application's own catalog, so it is never a
/// literal in the client.
///
/// The option of the page's locale is `selected` in the markup, so that the
/// control shows the right language before any client code runs — the form
/// submits it as it stands.
#[must_use]
#[allow(non_snake_case)]
pub fn LocaleOption(props: LocaleOptionProps) -> impl IntoView {
    ui::LocaleOption(props)
}
