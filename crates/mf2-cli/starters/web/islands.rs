use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use mf2::leptos::{CatalogPreload, IslandsGate, LocaleSwitcher, html_lang};

mf2::include_generated!();

/// The document. `lang` and `dir` are those of the language this request
/// is rendered in, so an Arabic page is right-to-left from its first byte.
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
                // No `<CatalogLinks/>`: a switch loads a new page, and the
                // server writes that page's preload.
                <CatalogPreload />
            </head>
            <body>
                // First, and outside every island: the islands after it
                // hydrate once the page's catalog is in.
                <IslandsGate />
                <App />
            </body>
        </html>
    }
}

/// The page renders on the server only, and so does the switcher: under
/// `static-locale` its form submits `?lang=`, and the server renders the
/// whole page in the new language.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text=tr!("app-title") />
        <header>
            <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply") />
        </header>
        <main>
            <h1>{tr!("greeting", name = "Ada")}</h1>
            <Visits />
        </main>
    }
}

/// The one part that runs in the browser.
#[island]
fn Visits() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}

/// The browser's entry point: what the build generated, then the catalog.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::hydrate_islands();
}

// The island `<IslandsGate/>` renders.
mf2::leptos::islands_gate!();
