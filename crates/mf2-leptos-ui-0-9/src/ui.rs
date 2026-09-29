//! The page's own i18n furniture — one source, compiled once per Leptos
//! line: `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8` both build this file,
//! each against its own line under the name `leptos`, so `view!` and
//! `#[component]` are used as in any Leptos application.
//!
//! These crates cannot depend on `mf2`, which re-exports them (Cargo forbids
//! the cycle). So each component is generic over a [`Layer`]: what it needs
//! from `mf2` — the languages, the page's own, the preload and the link
//! URLs, the client's switch. `mf2` implements it for a type of its own and
//! wraps each component in a function that names that type, so an
//! application writes `<LocaleSwitcher …>` as ever. The calls are resolved
//! when the component is compiled: nothing is installed at start-up, nothing
//! goes through a function pointer, and a page that renders none of the six
//! links none of them.
//!
//! WCAG 2.2 AA is a requirement here, not a nicety, and two of these
//! components exist because getting them right by hand is fiddly:
//!
//! * `<html lang dir>` must be correct and must **update on a switch**
//!   (3.1.1); `mf2`'s `html_lang` and `set_document_lang` do that.
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
//! (and the browser check greps the bundle for them).
//!
//! An earlier version rendered the options on the server only, on the theory
//! that a client view with no children would leave them alone. It does not:
//! tachys walks the cursor for the element's own end marker, finds an
//! `<option>` and traps the wasm, which took the rest of the page's
//! hydration with it. Server and client render the same tree.

use core::marker::PhantomData;
use std::string::String;
use std::vec::Vec;

use leptos::prelude::*;
use mf2_model::Dir;

/// What the components need from `mf2`, which they cannot name: `mf2`
/// implements it for a type of its own, and each component is generic over
/// it.
///
/// Every function is an associated function (no `self`), so a component
/// calls `L::locales()` directly: the call is resolved when the component is
/// compiled for `mf2`'s type.
pub trait Layer: 'static {
    /// The query parameter the switcher's `<select>` submits, which the
    /// server negotiates when no client code runs.
    const LOCALE_QUERY: &'static str;

    /// The island name [`IslandsGate`] renders; `mf2`'s `islands_gate!`
    /// exports the function of that name.
    const ISLANDS_GATE: &'static str;

    /// The `rel` of [`CatalogLinks`]' links, which the client's boot looks
    /// for.
    const CATALOG_LINK_REL: &'static str;

    /// Every locale this corpus was built for, with its base direction.
    fn locales() -> &'static [(&'static str, Dir)];

    /// The source locale: `x-default` for [`AlternateLinks`].
    fn source_locale() -> &'static str;

    /// The `lang` and `dir` of the page: the locale it is being rendered
    /// in, whose option [`LocaleOption`] marks `selected`.
    fn html_lang() -> (String, &'static str);

    /// The preload link for the catalog of the page's locale: its URL, and
    /// the reader's time zone the page was rendered in, if it was. A
    /// server's; `None` in a client build, which never renders it.
    fn preload() -> Option<(String, Option<String>)>;

    /// Every locale's catalog, as its tag and URL: the in-page map. A
    /// server's; empty in a client build, which never renders it.
    fn catalog_links() -> Vec<(&'static str, String)>;

    /// The client's live switch to `tag`: spawned, and on failure the page
    /// is left as it is. Nothing in a server build, where the form's own
    /// `GET` is the switch.
    fn switch(tag: String);
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
/// correcting. Absent, the page was rendered in the setup's zone.
#[component]
pub fn CatalogPreload<L: Layer>(
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        L::preload().map(|(href, zone)| {
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
/// no round trip to learn the hashed URL.
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
pub fn CatalogLinks<L: Layer>(
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let rel = L::CATALOG_LINK_REL;
        L::catalog_links()
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
/// Pair it with `mf2`'s `islands_gate!` in the client.
///
/// It must come before every island in document order, and outside all of
/// them. It has no content and no role, so assistive technology never meets
/// it.
#[component]
pub fn IslandsGate<L: Layer>(
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    let name = L::ISLANDS_GATE;
    view! { <leptos-island data-component=name></leptos-island> }
}

/// `<link rel="alternate" hreflang>` for a site whose locales have their own
/// URLs (a path prefix): `href_of` maps a tag to that locale's URL for the
/// page being rendered.
///
/// `x-default` points at the source locale, which is what a crawler uses when
/// it has no better match.
#[component]
pub fn AlternateLinks<L: Layer>(
    /// The URL of the current page in the given locale.
    href_of: fn(&str) -> String,
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    let source = L::source_locale();
    let mut links = Vec::new();
    for (tag, _) in L::locales() {
        links.push(view! { <link rel="alternate" hreflang=*tag href=href_of(tag) /> });
    }
    links.push(view! { <link rel="alternate" hreflang="x-default" href=href_of(source) /> });
    links
}

/// A labelled native control that switches locale, applied by a button.
///
/// A `<form method="get">` holding a `<select>` (named by the layer's query
/// parameter) inside its `<label>` and a submit button. Choosing a language
/// changes nothing until the button is pressed: the keyboard fires a
/// `<select>`'s `change` on every arrow key, so switching on it would change
/// the page's language — or, under `static-locale`, reload it — per keypress
/// (WCAG 3.2.2, failure F37).
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
#[component]
pub fn LocaleSwitcher<L: Layer>(
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
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    let select = NodeRef::<leptos::html::Select>::new();
    let query = L::LOCALE_QUERY;
    let on_submit = move |event: leptos::ev::SubmitEvent| switch_on_submit::<L>(&event, select);
    provide_context(SwitcherHref(href_of));
    view! {
        <form
            class="mf2-locale-switcher"
            method="get"
            on:submit=on_submit
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
pub fn LocaleOption<L: Layer>(
    /// The BCP 47 tag this option selects.
    tag: &'static str,
    /// The language's name **in that language** — from the application's own
    /// catalog, so it is never a literal in the client.
    children: Children,
    /// The layer the component reads: `mf2`'s.
    #[prop(optional)]
    _layer: PhantomData<L>,
) -> impl IntoView {
    let selected = L::html_lang().0 == tag;
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
fn switch_on_submit<L: Layer>(
    event: &leptos::ev::SubmitEvent,
    select: NodeRef<leptos::html::Select>,
) {
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
        event.prevent_default();
        L::switch(tag);
    }
}

/// The selected option's `data-mf2-href`, if it has one.
#[cfg(any(feature = "hydrate", feature = "csr"))]
fn selected_href(select: &web_sys::HtmlSelectElement) -> Option<String> {
    let index = u32::try_from(select.selected_index()).ok()?;
    let option = select.item(index)?;
    option.get_attribute("data-mf2-href")
}
