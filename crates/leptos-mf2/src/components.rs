//! The page's own i18n furniture (`plans/04-leptos-integration.md` §9).
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
//! locale's catalog — and never a literal in the client. The options render
//! on both sides, like any other description: what keeps the autonyms out of
//! the wasm is that they are catalog data, not that the client skips them
//! (B6, and the browser check greps the bundle for them).
//!
//! An earlier version rendered the options on the server only, on the theory
//! that a client view with no children would leave them alone. It does not:
//! tachys walks the cursor for the element's own end marker, finds an
//! `<option>` and traps the wasm (P0.10), which took the rest of the page's
//! hydration with it. Server and client render the same tree.

use alloc::string::String;
// The `view!` macro expands to `vec![…]` in the client build, and this crate
// is `no_std`; the server build of the same macro does not, hence the allow.
#[allow(unused_imports)]
use alloc::vec;
use alloc::vec::Vec;

use leptos::prelude::*;
use mf2_catalog::Dir;

#[cfg(feature = "ssr")]
use crate::links::catalog_href;
use crate::links::{CATALOG_LINK_REL, LOCALE_QUERY};

/// The `lang` and `dir` the shell should put on `<html>`: the catalog's own
/// locale, so the page always says what it is in (WCAG 3.1.1).
///
/// With no catalog — before boot, or a build with none — it is the source
/// locale and `ltr`.
#[must_use]
pub fn html_lang() -> (String, &'static str) {
    match crate::catalog::active() {
        Some(catalog) => (
            catalog.locale().into(),
            if catalog.dir() == Dir::Rtl {
                "rtl"
            } else {
                "ltr"
            },
        ),
        None => (crate::state::source_locale().into(), "ltr"),
    }
}

/// The preload link for the catalog of the locale this page is being
/// rendered in — the boot data, and the reason the client needs no inline
/// script and makes no extra request (§6).
///
/// It renders only on the server: the client reads this link, it does not
/// write it, and the shell's `<head>` is not hydrated.
///
/// When the request was rendered in the reader's time zone, the link says
/// which (`data-mf2-zone`), so that the client knows whether its dates need
/// correcting (`plans/03-runtime.md` §6.1). Absent, the page was rendered in
/// `Setup`'s zone.
#[component]
pub fn CatalogPreload() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let href = crate::catalog::active()
            .and_then(|catalog| crate::catalog::catalog_name(catalog.locale()))
            .map(catalog_href);
        let zone = crate::catalog::request_time_zone()
            .and_then(|zone| crate::zone::zone_name(&zone).map(String::from));
        href.map(|href| {
            view! {
                <link
                    rel="preload"
                    r#as="fetch"
                    crossorigin="anonymous"
                    href=href
                    data-mf2=""
                    data-mf2-zone=zone
                />
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {}
}

/// The in-page tag → URL map: one `<link>` per locale, so that a switch needs
/// no round trip to learn the hashed URL (§6, owner question 1).
///
/// A site that would rather keep its pages a few bytes smaller simply does
/// not render this, and the switch redirects through `/i18n/<tag>` instead.
///
/// **Every** locale, including the one the page was rendered in. Leaving it
/// out looks like a saving — the preload link already carries it — but the
/// preload is the *page's* locale, and after one switch that is no longer
/// the locale the user may want back. Conformance L6 in the browser found
/// this by switching to the twin and failing to come home.
#[component]
pub fn CatalogLinks() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let links: Vec<_> = crate::catalog::catalog_entries()
            .iter()
            .map(|entry| {
                view! {
                    <link
                        rel=CATALOG_LINK_REL
                        data-mf2-locale=entry.tag
                        href=catalog_href(entry.file)
                    />
                }
            })
            .collect();
        links
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = CATALOG_LINK_REL;
    }
}

/// The first thing in an islands page's `<body>`: an empty island that
/// Leptos' island walk awaits until the catalog is installed, so that every
/// island after it hydrates against the catalog the page was rendered with
/// (`hydrate_islands`, with `hydrate`, says why nothing else can wait).
/// Pair it with [`islands_gate!`](crate::islands_gate) in the client.
///
/// It must come before every island in document order, and outside all of
/// them. It has no content and no role, so assistive technology never meets
/// it.
#[component]
pub fn IslandsGate() -> impl IntoView {
    view! { <leptos-island data-component=crate::links::ISLANDS_GATE></leptos-island> }
}

/// `<link rel="alternate" hreflang>` for a site whose locales have their own
/// URLs (a path prefix): `href_of` maps a tag to that locale's URL for the
/// page being rendered.
///
/// `x-default` points at the source locale, which is what a crawler uses when
/// it has no better match.
#[component]
pub fn AlternateLinks(
    /// The URL of the current page in the given locale.
    href_of: fn(&str) -> String,
) -> impl IntoView {
    let source = crate::state::source_locale();
    let mut links = Vec::new();
    for (tag, _) in crate::state::locales() {
        links.push(view! { <link rel="alternate" hreflang=*tag href=href_of(tag) /> });
    }
    links.push(view! { <link rel="alternate" hreflang="x-default" href=href_of(source) /> });
    links
}

/// A labelled native control that switches locale, applied by a button.
///
/// A `<form method="get">` holding a `<select name=`[`LOCALE_QUERY`]`>`
/// inside its `<label>` and a submit button. Choosing a language changes
/// nothing until the button is pressed: the keyboard fires a `<select>`'s
/// `change` on every arrow key, so switching on it would change the page's
/// language — or, under `static-locale`, reload it — per keypress (WCAG
/// 3.2.2, failure F37; §9).
///
/// With no client code — before the wasm loads, after a failed boot, or on
/// an islands page where the switcher is not an island — the form's own
/// `GET ?lang=…` is the switch, which `mf2-axum`'s `QueryParam` negotiates.
/// Under `hydrate` and `csr` the submit is intercepted and becomes
/// [`set_locale`](crate::set_locale), live, with focus left on the button.
///
/// The `<select>` is inside its `<label>`, so there is no fixed `id` and a
/// page may carry two switchers (a header and a footer). The options are
/// [`LocaleOption`]s the caller writes, each carrying its own `lang`.
///
/// ```ignore
/// <LocaleSwitcher label=tr!("choose-language") button=tr!("apply-language")>
///     <LocaleOption tag="en">{tr!("language-en")}</LocaleOption>
///     <LocaleOption tag="fr">{tr!("language-fr")}</LocaleOption>
/// </LocaleSwitcher>
/// ```
#[component]
pub fn LocaleSwitcher(
    /// The control's accessible name. A `<label>`, not a `placeholder` and
    /// not an `aria-label` on its own, so that it is visible as well as
    /// announced (WCAG 3.3.2).
    #[prop(into)]
    label: TextProp,
    /// The submit button's text — the application's own message, in the
    /// page's language, like `label`.
    #[prop(into)]
    button: TextProp,
    /// The options: one [`LocaleOption`] per locale offered.
    children: Children,
) -> impl IntoView {
    let select = NodeRef::<leptos::html::Select>::new();
    view! {
        <form
            class="mf2-locale-switcher"
            method="get"
            on:submit=move |event| switch_on_submit(&event, select)
        >
            <label>
                <span>{move || label.get()}</span>
                <select name=LOCALE_QUERY node_ref=select>
                    {children()}
                </select>
            </label>
            <button type="submit">{move || button.get()}</button>
        </form>
    }
}

/// One `<option>`, named in its own language and marked as being in it.
///
/// The option of the page's locale is `selected` in the markup, so that the
/// control shows the right language before any client code runs — the form
/// submits it as it stands.
#[component]
pub fn LocaleOption(
    /// The BCP 47 tag this option selects.
    tag: &'static str,
    /// The language's name **in that language** — from the application's own
    /// catalog, so it is never a literal in the client.
    children: Children,
) -> impl IntoView {
    let selected = html_lang().0 == tag;
    view! {
        <option value=tag lang=tag selected=selected>
            {children()}
        </option>
    }
}

/// What the form's submit does on the client: switch in place, and on
/// failure leave the page alone. Without client code the browser submits the
/// form, and the server negotiates `?lang=`.
#[allow(unused_variables)]
fn switch_on_submit(event: &leptos::ev::SubmitEvent, select: NodeRef<leptos::html::Select>) {
    #[cfg(any(feature = "hydrate", feature = "csr"))]
    {
        let Some(element) = select.get_untracked() else {
            return;
        };
        let Some(tag) =
            js_sys::Reflect::get(element.as_ref(), &wasm_bindgen::JsValue::from_str("value"))
                .ok()
                .and_then(|value| value.as_string())
        else {
            return;
        };
        event.prevent_default();
        leptos::task::spawn_local(async move {
            if let Err(error) = crate::set_locale(&tag).await {
                web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(match error {
                    crate::LoadError::UnknownLocale => "mf2: no such locale",
                    _ => "mf2: the locale could not be switched; the page is unchanged",
                }));
            }
        });
    }
}
