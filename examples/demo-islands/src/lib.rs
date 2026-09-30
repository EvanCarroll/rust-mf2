//! The Phase 7 islands example (`plans/15-phase-7-work-order.md` A1).
//!
//! Most of this page is **server-only**: it renders on the server and ships
//! no code, so its call sites cost the wasm nothing — `cargo xtask
//! islands-zero` measures that. One part is an island, and hydrates:
//!
//! | On the page | Where it runs | What it exercises |
//! |---|---|---|
//! | the `<title>`, heading, tagline, note | server | plain text |
//! | the search field's `placeholder` | server | a description as an attribute |
//! | the hotkey line | server | markup as elements, with no client code |
//! | the counter | **island** | a signal-valued argument under `static-locale`: the one kind of node that still registers |
//! | the note above the counter | **island** | a markup message in an island, whose node structure comes from the catalog — the case [`IslandsGate`] exists for |
//! | the switcher | server | `static-locale`'s switch: the form's `GET ?lang=`, which the server negotiates and remembers in the cookie |
//!
//! Layout is flexbox, the SVG is an external file, and the page carries
//! schema.org `inLanguage`.

use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher, html_lang};

use demo_islands_i18n::{Locale, tr};

/// The document. Islands mode: the scripts hydrate islands, not the body.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let (lang, dir) = html_lang();
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options islands=true />
                <MetaTags />
                <link rel="stylesheet" href="/pkg/demo_islands.css" />
                <link rel="icon" href="/globe.svg" type="image/svg+xml" />
                // The boot data, downloading in parallel with the wasm (§6).
                // No per-locale map: under `static-locale` a switch is a
                // reload, and the server writes the new page's preload.
                <CatalogPreload />
            </head>
            <body>
                // First, and outside every island: Leptos' island walk waits
                // here until the catalog is installed.
                <IslandsGate />
                <App />
            </body>
        </html>
    }
}

/// The page. A component, not an island: it renders on the server only.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text=tr!("app-title") />

        // The landmarks are siblings — banner, main, contentinfo — so that
        // "skip to main" lands on the content (WCAG 1.3.1, 2.4.1).
        <div class="page" itemscope itemtype="https://schema.org/WebPage">
            <meta itemprop="inLanguage" content=html_lang().0 />
            <header class="row">
                <h1 itemprop="name">{tr!("app-title")}</h1>
                <Switcher />
            </header>

            <main>
                <p class="tagline">{tr!("tagline")}</p>

                <section class="card">
                    <p id="server-note">{tr!("server-note")}</p>
                    <label class="field">
                        <span>{tr!("search-label")}</span>
                        <input type="search" placeholder=tr!("search-placeholder") />
                    </label>
                    // Markup on the server: real elements, no client code.
                    <p id="hotkey">
                        {tr!("hotkey", kbd = |children| view! { <kbd>{children}</kbd> })}
                    </p>
                </section>

                <Counter />

                {more_server()}
            </main>

            <footer class="row">
                <img src="/globe.svg" alt="" width="16" height="16" />
                <small>{tr!("tagline")}</small>
            </footer>
        </div>
    }
}

/// An island: the counter, and the markup message above it.
///
/// The note is the case the gate exists for. Its `<em>` is a node the
/// catalog describes, so hydrating it without the catalog would walk a
/// structure the island does not know.
#[island]
fn Counter() -> impl IntoView {
    let count = RwSignal::new(3);
    view! {
        <section class="card">
            <p id="island-note">
                {tr!("island-note", em = |children| view! { <em>{children}</em> })}
            </p>
            // Signal-valued: under `static-locale` this is the one node that
            // registers, because its argument effect needs somewhere to live.
            // `role="status"`: the count changes with focus on the button, so
            // it is announced without moving focus (WCAG 4.1.3).
            <p id="people" role="status">{tr!("people-online", count = count)}</p>
            <div class="row">
                <button id="add-one" on:click=move |_| *count.write() += 1>
                    {tr!("add-one")}
                </button>
                <button id="reset" on:click=move |_| count.set(0)>
                    {tr!("reset")}
                </button>
            </div>
        </section>
    }
}

/// The switcher — **not** an island. Its form's `GET ?lang=` is the switch
/// under `static-locale`: the server negotiates the query, writes the cookie
/// and renders the whole page, server-only parts included, in the new
/// locale. So it ships no code, and works before the wasm loads.
#[component]
fn Switcher() -> impl IntoView {
    view! {
        <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
            <LocaleOption tag=Locale::En>{Locale::En.name()}</LocaleOption>
            <LocaleOption tag=Locale::Fr>{Locale::Fr.name()}</LocaleOption>
            <LocaleOption tag=Locale::Ar>{Locale::Ar.name()}</LocaleOption>
        </LocaleSwitcher>
    }
}

/// The measured addition: a server-only component with a call site in every
/// position — text, attribute, argument, markup — compiled in only with
/// `more-server`.
#[cfg(feature = "more-server")]
fn more_server() -> impl IntoView {
    view! {
        <section class="card" id="more-server">
            <h2>{tr!("tagline")}</h2>
            <p>{tr!("server-note")}</p>
            <input type="search" placeholder=tr!("search-placeholder") />
            <p>{tr!("people-online", count = 12)}</p>
            <p>{tr!("hotkey", kbd = |children| view! { <kbd>{children}</kbd> })}</p>
            <p>{tr!("island-note", em = |children| view! { <strong>{children}</strong> })}</p>
        </section>
    }
}

#[cfg(not(feature = "more-server"))]
fn more_server() -> impl IntoView {}

/// How many nodes follow the locale. Under `static-locale` that is only the
/// counter's line, so the browser check reads `1` as "the counter island has
/// hydrated".
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    leptos_mf2::live_nodes()
}

/// The client's entry point: install the generated setup, start the catalog
/// load, and set the owner the island walk needs — synchronously, because
/// Leptos' script does not wait for this function.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    demo_islands_i18n::install();
    leptos_mf2::hydrate_islands();
}

// The island `IslandsGate` renders: the walk awaits it until the catalog is
// installed.
leptos_mf2::islands_gate!();
