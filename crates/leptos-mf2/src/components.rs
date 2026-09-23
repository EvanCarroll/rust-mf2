//! The page's own i18n furniture (`plans/04-leptos-integration.md` §9).
//!
//! WCAG 2.2 AA is a requirement here, not a nicety, and two of these
//! components exist because getting them right by hand is fiddly:
//!
//! * `<html lang dir>` must be correct and must **update on a switch**
//!   (3.1.1). [`set_document_lang`](crate::set_document_lang) does the
//!   update; [`html_lang`] gives the shell the pair to render.
//! * A language control must be a real labelled control, and each language
//!   must be named **in its own language, with its own `lang`** — otherwise
//!   a screen reader pronounces "Français" with an English voice.
//!
//! **Where the option text comes from, and why it is not in the wasm.**
//! [`LocaleOption`] takes its text as children, so the names come from the
//! application's own catalog (one message per locale, present in every
//! locale's catalog) — never from a literal in the client. The server
//! renders the `<option>` elements; the client's view of the `<select>`
//! declares **no** children, so hydration attaches the change handler and
//! leaves the options exactly as they were served. No autonym reaches the
//! wasm, which is what the size canaries check for (B6).

use alloc::string::String;
use alloc::vec::Vec;

use leptos::prelude::*;
use mf2_catalog::Dir;

use crate::links::CATALOG_LINK_REL;
#[cfg(feature = "ssr")]
use crate::links::catalog_href;

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
#[component]
pub fn CatalogPreload() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let href = crate::catalog::active()
            .and_then(|catalog| crate::catalog::catalog_name(catalog.locale()))
            .map(catalog_href);
        href.map(|href| {
            view! {
                <link
                    rel="preload"
                    r#as="fetch"
                    crossorigin="anonymous"
                    href=href
                    data-mf2=""
                />
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {}
}

/// The in-page tag → URL map: one `<link>` per **other** locale, so that a
/// switch needs no round trip to learn the hashed URL (§6, owner question 1).
///
/// A site that would rather keep its pages a few bytes smaller simply does
/// not render this, and the switch redirects through `/i18n/<tag>` instead.
#[component]
pub fn CatalogLinks() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let here = crate::catalog::active().map(|c| String::from(c.locale()));
        let links: Vec<_> = crate::catalog::catalog_entries()
            .iter()
            .filter(|entry| here.as_deref() != Some(entry.tag))
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

/// A labelled native control that switches locale.
///
/// The control is a `<select>` — a real form control, so it is reachable by
/// keyboard, announced with its label, and styled by the platform. The
/// options are [`LocaleOption`]s the caller writes, each carrying its own
/// `lang`.
///
/// ```ignore
/// <LocaleSwitcher label=tr!("choose-language")>
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
    /// The options: one [`LocaleOption`] per locale offered.
    children: Children,
) -> impl IntoView {
    let (lang, _) = html_lang();
    view! {
        <div class="mf2-locale-switcher">
            <label for="mf2-locale">{move || label.get()}</label>
            <select
                id="mf2-locale"
                name="mf2-locale"
                on:change=|event| switch_on_change(&event)
                prop:value=lang.clone()
            >
                {options(children)}
            </select>
        </div>
    }
}

/// The options render on the server only: their text is locale data, and
/// hydrating a `<select>` whose client view has no children leaves the
/// served options exactly as they are.
#[cfg(feature = "ssr")]
fn options(children: Children) -> impl IntoView {
    children()
}

#[cfg(not(feature = "ssr"))]
fn options(children: Children) -> impl IntoView {
    drop(children);
}

/// One `<option>`, named in its own language and marked as being in it.
#[component]
pub fn LocaleOption(
    /// The BCP 47 tag this option selects.
    tag: &'static str,
    /// The language's name **in that language** — from the application's own
    /// catalog, so it is never a literal in the client.
    children: Children,
) -> impl IntoView {
    view! {
        <option value=tag lang=tag>
            {children()}
        </option>
    }
}

/// What the `<select>` does: switch, and on failure leave the page alone.
#[allow(unused_variables)]
fn switch_on_change(event: &leptos::ev::Event) {
    #[cfg(any(feature = "hydrate", feature = "csr"))]
    {
        let tag = event_target_value(event);
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
