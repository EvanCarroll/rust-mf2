//! The page's own i18n furniture (`plans/04-leptos-integration.md` §9) — one
//! source, compiled once per Leptos line: `mf2-leptos-ui-0-9` and
//! `mf2-leptos-ui-0-8` both build this file, each against its own line under
//! the name `leptos`, so `view!` and `#[component]` are used as normal
//! (Phase 10 A7; owner question 13).
//!
//! These crates cannot depend on the i18n layer that re-exports them (Cargo
//! forbids the cycle), so the layer installs a [`Table`] of what the
//! components need from it — the languages, the current one, the preload
//! and the link URLs ([`install`]) — and the client's switch
//! (`install_switch`, with `hydrate` or `csr`). The
//! layer re-exports each component as a plain function that installs them
//! and returns this crate's view, so an application that renders none links
//! neither.
//!
//! WCAG 2.2 AA is a requirement here, not a nicety, and two of these
//! components exist because getting them right by hand is fiddly:
//!
//! * `<html lang dir>` must be correct and must **update on a switch**
//!   (3.1.1): the layer's `html_lang` and `set_document_lang` do that.
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

use std::string::String;
use std::sync::OnceLock;
use std::vec::Vec;

use leptos::prelude::*;
use mf2_model::Dir;

/// What the components need from the i18n layer, which they cannot name:
/// installed once, at start-up, by the layer ([`install`]).
///
/// Every field is either a `'static` name or a function the layer owns, so
/// the table itself is a `static` of the layer's, written as a struct
/// literal (its mode-specific fields are `cfg`'d, which a constructor's
/// arguments cannot be).
#[derive(Clone, Copy)]
pub struct Table {
    /// Every locale this corpus was built for, with its base direction.
    pub locales: fn() -> &'static [(&'static str, Dir)],
    /// The source locale: `x-default` for [`AlternateLinks`].
    pub source_locale: fn() -> &'static str,
    /// Whether `tag` is the locale the page is being rendered in — the
    /// option [`LocaleOption`] marks `selected`.
    pub is_current: fn(&str) -> bool,
    /// The query parameter the switcher's `<select>` submits, which the
    /// server negotiates when no client code runs.
    pub locale_query: &'static str,
    /// The island name [`IslandsGate`] renders; the layer's
    /// `islands_gate!` exports the function of that name.
    pub islands_gate: &'static str,
    /// The server's preload link for the page's own catalog: its URL, and
    /// the reader's time zone the page was rendered in, if it was.
    #[cfg(feature = "ssr")]
    pub preload: fn() -> Option<(String, Option<String>)>,
    /// The server's in-page tag → URL map: every locale's catalog URL.
    #[cfg(feature = "ssr")]
    pub catalog_links: fn() -> Vec<(&'static str, String)>,
    /// The `rel` of those links, which the client's boot looks for.
    #[cfg(feature = "ssr")]
    pub catalog_link_rel: &'static str,
}

/// The installed table.
static TABLE: OnceLock<&'static Table> = OnceLock::new();

/// Installs the layer's table. The i18n layer calls it from its own
/// `install`, before anything renders; a second call is ignored.
pub fn install(table: &'static Table) {
    let _ = TABLE.set(table);
}

/// The installed table, or `None` before [`install`] — in which state every
/// component renders what it would with no languages, and nothing panics.
fn table() -> Option<&'static Table> {
    TABLE.get().copied()
}

/// The client's live switch. Not a [`Table`] field: every component
/// installs the table, and the switch reaches the catalog fetch, the node
/// registry's walk, the cookie and the address — about 4.4 KB gz of client
/// (Phase 10 A7, measured) that a page with options or a preload link but no
/// switcher would carry. The layer's `LocaleSwitcher` installs it, so only
/// an application that renders one links it, as in 1.x.
#[cfg(any(feature = "hydrate", feature = "csr"))]
static SWITCH: OnceLock<fn(String)> = OnceLock::new();

/// Installs the client's live switch to a tag: spawned, and on failure the
/// page is left alone. A second call is ignored.
#[cfg(any(feature = "hydrate", feature = "csr"))]
pub fn install_switch(switch: fn(String)) {
    let _ = SWITCH.set(switch);
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
        table().and_then(|t| (t.preload)()).map(|(href, zone)| {
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
        let (rel, links) = match table() {
            Some(t) => (t.catalog_link_rel, (t.catalog_links)()),
            None => ("", Vec::new()),
        };
        links
            .into_iter()
            .map(|(tag, href)| {
                view! { <link rel=rel data-mf2-locale=tag href=href /> }
            })
            .collect::<Vec<_>>()
    }
    #[cfg(not(feature = "ssr"))]
    {}
}

/// The first thing in an islands page's `<body>`: an empty island that
/// Leptos' island walk awaits until the catalog is installed, so that every
/// island after it hydrates against the catalog the page was rendered with.
/// Pair it with the layer's `islands_gate!` in the client.
///
/// It must come before every island in document order, and outside all of
/// them. It has no content and no role, so assistive technology never meets
/// it.
#[component]
pub fn IslandsGate() -> impl IntoView {
    let name = table().map_or("", |t| t.islands_gate);
    view! { <leptos-island data-component=name></leptos-island> }
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
    let (locales, source) = match table() {
        Some(t) => ((t.locales)(), (t.source_locale)()),
        None => (&[][..], ""),
    };
    let mut links = Vec::new();
    for (tag, _) in locales {
        links.push(view! { <link rel="alternate" hreflang=*tag href=href_of(tag) /> });
    }
    links.push(view! { <link rel="alternate" hreflang="x-default" href=href_of(source) /> });
    links
}

/// A labelled native control that switches locale, applied by a button.
///
/// A `<form method="get">` holding a `<select name="lang">` (the installed
/// query parameter) inside its `<label>` and a submit button. Choosing a
/// language changes nothing until the button is pressed: the keyboard fires
/// a `<select>`'s `change` on every arrow key, so switching on it would
/// change the page's language — or, under `static-locale`, reload it — per
/// keypress (WCAG 3.2.2, failure F37; §9).
///
/// With no client code — before the wasm loads, after a failed boot, or on
/// an islands page where the switcher is not an island — the form's own
/// `GET ?lang=…` is the switch, which the server negotiates. Under `hydrate`
/// and `csr` the submit is intercepted and becomes the layer's live switch,
/// with focus left on the button.
///
/// The `<select>` is inside its `<label>`, so there is no fixed `id` and a
/// page may carry two switchers (a header and a footer). The options are
/// [`LocaleOption`]s the caller writes, each carrying its own `lang`.
///
/// **A site whose languages live in its URLs** (`/fr/…`, a path prefix)
/// passes `href_of`, the shape [`AlternateLinks`] takes: each option then
/// carries its language's URL as `data-mf2-href`, and the submit navigates
/// there instead of switching in place — a `?lang=` cannot outrank the
/// path. Without the wasm the form's `?lang=` still goes to the server,
/// which sends it on to that URL.
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
    /// For a site whose languages live in its URLs: the URL of the current
    /// page in the given locale. The submit then navigates there.
    #[prop(optional)]
    href_of: Option<fn(&str) -> String>,
) -> impl IntoView {
    let select = NodeRef::<leptos::html::Select>::new();
    let query = table().map_or("", |t| t.locale_query);
    provide_context(SwitcherHref(href_of));
    view! {
        <form
            class="mf2-locale-switcher"
            method="get"
            on:submit=move |event| switch_on_submit(&event, select)
        >
            <label>
                <span>{move || label.get()}</span>
                <select name=query node_ref=select>
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
    let selected = table().is_some_and(|t| (t.is_current)(tag));
    let href = use_context::<SwitcherHref>().and_then(|SwitcherHref(f)| f.map(|f| f(tag)));
    view! {
        <option value=tag lang=tag selected=selected data-mf2-href=href>
            {children()}
        </option>
    }
}

/// The enclosing switcher's `href_of`, for its options.
#[derive(Clone, Copy)]
struct SwitcherHref(Option<fn(&str) -> String>);

/// What the form's submit does on the client: switch in place, and on
/// failure leave the page alone. Without client code the browser submits the
/// form, and the server negotiates `?lang=`.
#[allow(unused_variables, clippy::needless_pass_by_value)]
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
        // A site whose languages live in its URLs: go to the chosen
        // option's URL, where the server renders the page in its language.
        if let Some(href) = selected_href(&element) {
            event.prevent_default();
            if let Some(window) = web_sys::window() {
                let _ = window.location().assign(&href);
            }
            return;
        }
        let Some(switch) = SWITCH.get() else {
            return;
        };
        event.prevent_default();
        switch(tag);
    }
}

/// The selected option's `data-mf2-href`, if it has one.
#[cfg(any(feature = "hydrate", feature = "csr"))]
fn selected_href(select: &web_sys::HtmlSelectElement) -> Option<String> {
    let index = u32::try_from(select.selected_index()).ok()?;
    let option = select.item(index)?;
    option.get_attribute("data-mf2-href")
}
