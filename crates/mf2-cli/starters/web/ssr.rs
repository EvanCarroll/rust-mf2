use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use mf2::leptos::{CatalogLinks, CatalogPreload, LocaleSwitcher, html_lang};

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
                <HydrationScripts options />
                <MetaTags />
                <CatalogPreload />
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
    view! {
        <Title text=tr!("app-title") />
        <Router>
            <header>
                <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply") />
            </header>
            <main>
                <Routes fallback=|| view! { <p>{tr!("not-found")}</p> }>
                    <Route path=path!("/") view=Home />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Home() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <h1>{tr!("greeting", name = "Ada")}</h1>
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}

/// The browser's entry point.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::hydrate_body(App);
}
