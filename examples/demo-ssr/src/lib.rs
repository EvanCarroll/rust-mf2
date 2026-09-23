//! The Phase 6 example (`plans/14-phase-6-work-order.md` A10).
//!
//! One page that uses every position the integration supports, so that what
//! breaks is visible rather than theoretical:
//!
//! | On the page | What it exercises |
//! |---|---|
//! | the `<title>` | D9 — leptos_meta reads it *after* rendering, outside the request owner (04 §5) |
//! | the heading and the tagline | a `simple` message: tachys writes the catalog's borrowed `&str` |
//! | the search field's `placeholder` | a description as an attribute value |
//! | "N people are here" | a **signal-valued** argument: one library effect, no closure per call site (04 §4) |
//! | the hotkey line | **markup as elements** — and the `<kbd>` lands in a different place in French, which is the point (04 §7) |
//! | the published line | a date, through `:datetime` and the catalog's ICU4X blob |
//! | the echo line | a plain `String` from an event handler — no bidi isolation in it (04 §9) |
//! | the switcher | `<LocaleSwitcher>`: a labelled native control whose option text never reaches the wasm |
//!
//! Layout is flexbox, the SVG is an external file, and the page carries
//! schema.org `inLanguage` so that the locale is machine-readable as well as
//! rendered.

use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};
use mf2::DateTimeValue;

/// The document. `<html lang dir>` comes from the catalog this request is
/// being rendered with, so the page always says what language it is in
/// (WCAG 3.1.1) and an RTL locale lays out right-to-left without any other
/// change.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let (lang, dir) = html_lang();
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
                <link rel="stylesheet" href="/pkg/demo_ssr.css" />
                <link rel="icon" href="/globe.svg" type="image/svg+xml" />
                // The boot data: the catalog's immutable URL, preloaded so
                // that it downloads in parallel with the wasm (§6).
                <CatalogPreload />
                // The other locales' URLs, so a switch costs no round trip.
                <CatalogLinks />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let count = RwSignal::new(3);
    let typed = RwSignal::new(String::new());
    // A fixed instant, so that the page is the same on every run and the
    // browser checks can compare it.
    let published = DateTimeValue::instant(1_767_225_600_000);

    view! {
        // leptos_meta evaluates this *after* rendering, outside the request
        // owner — the case P0.2 found, and what the `ssr` capture fixes.
        <Title text=demo_i18n::tr!("app-title") />

        <main class="page" itemscope itemtype="https://schema.org/WebPage">
            // schema.org: the page states its language as data as well as
            // rendering it, so a crawler and a screen reader agree.
            <meta itemprop="inLanguage" content=html_lang().0 />
            <header class="row">
                <h1 itemprop="name">{demo_i18n::tr!("app-title")}</h1>
                <LocaleSwitcher label=demo_i18n::tr!("language.label")>
                    <LocaleOption tag="en">{demo_i18n::tr!("language.en")}</LocaleOption>
                    <LocaleOption tag="fr">{demo_i18n::tr!("language.fr")}</LocaleOption>
                    <LocaleOption tag="ar">{demo_i18n::tr!("language.ar")}</LocaleOption>
                </LocaleSwitcher>
            </header>

            <p class="tagline">{demo_i18n::tr!("tagline")}</p>

            <section class="card">
                <label class="field">
                    <span>{demo_i18n::tr!("search-label")}</span>
                    // A description as an attribute value: one function in
                    // the library, not one per call site.
                    <input type="search" placeholder=demo_i18n::tr!("search-placeholder") />
                </label>
            </section>

            <section class="card">
                // A signal-valued argument. The count is reactive and the
                // locale is reactive, and neither costs this call site a
                // closure: `count` goes in as a `Signal`, and the node's
                // own argument effect is the library's (04 §4).
                <p id="people">{demo_i18n::tr!("people-online", count = count)}</p>
                <div class="row">
                    <button id="add-one" on:click=move |_| *count.write() += 1>
                        {demo_i18n::tr!("add-one")}
                    </button>
                    <button id="reset" on:click=move |_| count.set(0)>
                        {demo_i18n::tr!("reset")}
                    </button>
                </div>
            </section>

            <section class="card">
                // Markup as elements: `{#kbd}…{/kbd}` becomes a real <kbd>,
                // and in French it lands at the end of the sentence instead
                // of the middle — without the view knowing anything about
                // word order.
                <p id="hotkey">
                    {demo_i18n::tr!("hotkey", kbd = |children: AnyView| view! { <kbd>{children}</kbd> })}
                </p>
                <p id="published">
                    {published
                        .clone()
                        .map(|when| demo_i18n::tr!("published", when = when))}
                </p>
            </section>

            <section class="card">
                <label class="field">
                    <span>{demo_i18n::tr!("search-label")}</span>
                    <input
                        id="echo-input"
                        type="text"
                        on:input=move |event| typed.set(event_target_value(&event))
                    />
                </label>
                // A plain `String`, built in an event handler and shown as
                // data: no bidi isolation in it, because a program — and a
                // comparison — consumes it (04 §9).
                <p id="echo">{move || echo(typed.get())}</p>
            </section>

            <footer class="row">
                <img src="/globe.svg" alt="" width="16" height="16" />
                <small>{demo_i18n::tr!("tagline")}</small>
            </footer>
        </main>
    }
}

/// The `String` path — about half of real call sites (04 §2) — here in an
/// event-driven position: no view, no signal, just text.
fn echo(typed: String) -> String {
    if typed.is_empty() {
        demo_i18n::tr!("typed-nothing").to_string()
    } else {
        demo_i18n::tr!("typed", typed = typed).to_string()
    }
}

/// The client's entry point: install the generated setup, then boot.
///
/// `hydrate_body` fetches the catalog the page was served with — reusing the
/// preload, so no extra request — validates it against `MANIFEST_HASH`,
/// installs it, and only then hydrates (§6).
/// How many nodes follow the locale (D7's registry). The browser checks poll
/// it to know that hydration has finished, and read it again after a route
/// change to see that dropped nodes freed their slots.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    leptos_mf2::live_nodes()
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(demo_i18n::setup());
    leptos_mf2::hydrate_body(App);
}
