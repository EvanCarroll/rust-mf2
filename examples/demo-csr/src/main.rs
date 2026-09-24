//! The Phase 7 client-only example (`plans/15-phase-7-work-order.md` A2).
//!
//! There is no server: trunk builds the wasm, `mf2 compile --site` publishes
//! the catalogs beside it, and any static host serves the lot. The locale is
//! chosen in the browser — the one remembered from last time, else the
//! reader's languages, else English — and a switch is live, and remembered.
//!
//! | On the page | What it exercises |
//! |---|---|
//! | the `<title>` | a description through `TextProp`, following a switch |
//! | heading, tagline | plain text |
//! | the search field's `placeholder` | a description as an attribute |
//! | the hotkey line | markup as elements: the reason nothing mounts before the catalog |
//! | the counter | a signal-valued argument |
//! | the switcher | a live switch, remembered in `localStorage` |
//!
//! Layout is flexbox, the SVG is an external file, and the page carries
//! schema.org `inLanguage`, kept up to date across a switch.

use leptos::prelude::*;
use leptos_meta::{Title, provide_meta_context};
use leptos_mf2::{LocaleOption, LocaleSwitcher, html_lang};

use demo_csr_i18n::tr;

/// The page.
#[component]
fn App() -> impl IntoView {
    provide_meta_context();
    // `html_lang` reads the active catalog, which is not a signal; the
    // trigger a switch fires is what makes this follow it (`track_locale`
    // subscribes until the reader's next run, so nothing is left behind).
    let in_language = move || {
        leptos_mf2::track_locale();
        html_lang().0
    };
    let count = RwSignal::new(3);
    view! {
        <Title text=tr!("app-title") />

        <main class="page" itemscope itemtype="https://schema.org/WebPage">
            <meta itemprop="inLanguage" content=in_language />
            <header class="row">
                <h1 itemprop="name">{tr!("app-title")}</h1>
                <LocaleSwitcher label=tr!("language.label")>
                    <LocaleOption tag="en">{tr!("language.en")}</LocaleOption>
                    <LocaleOption tag="fr">{tr!("language.fr")}</LocaleOption>
                    <LocaleOption tag="ar">{tr!("language.ar")}</LocaleOption>
                </LocaleSwitcher>
            </header>

            <p class="tagline" id="tagline">{tr!("tagline")}</p>

            <section class="card">
                <label class="field">
                    <span>{tr!("search-label")}</span>
                    <input id="search" type="search" placeholder=tr!("search-placeholder") />
                </label>
                <p id="hotkey">
                    {tr!("hotkey", kbd = |children: AnyView| view! { <kbd>{children}</kbd> })}
                </p>
            </section>

            <section class="card">
                <p id="people">{tr!("people-online", count = count)}</p>
                <div class="row">
                    <button id="add-one" on:click=move |_| *count.write() += 1>
                        {tr!("add-one")}
                    </button>
                    <button id="reset" on:click=move |_| count.set(0)>
                        {tr!("reset")}
                    </button>
                </div>
            </section>

            <footer class="row">
                <img src="globe.svg" alt="" width="16" height="16" />
                <small>{tr!("tagline")}</small>
            </footer>
        </main>
    }
}

/// How many nodes follow the locale — the number the browser check reads to
/// know that the page mounted, and that a switch reached every node.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    leptos_mf2::live_nodes()
}

fn main() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(demo_csr_i18n::setup());
    // Chooses the locale, loads the index and that locale's catalog, and
    // only then mounts.
    leptos_mf2::mount_to_body(App);
}
