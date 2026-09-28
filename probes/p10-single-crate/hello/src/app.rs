//! The document shell and the page. Declared after the include.

use crate::pages::{Visits, not_found};
use crate::tr;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::{Lazy, path};

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
        <Title text=crate::title() />
        <Router>
            <header>
                <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
                    <LocaleOption tag="en">{tr!("language.en")}</LocaleOption>
                    <LocaleOption tag="fr">{tr!("language.fr")}</LocaleOption>
                </LocaleSwitcher>
            </header>
            <nav>
                <A href="/">{tr!("app-title")}</A>
                " "
                <A href="/visits">{tr!("visit-again")}</A>
            </nav>
            <main>
                <Routes fallback=not_found>
                    <Route path=path!("/") view=|| view! { <h1>{tr!("greeting", name = "Ada")}</h1> } />
                    <Route path=path!("/visits") view={Lazy::<Visits>::new()} />
                </Routes>
            </main>
        </Router>
    }
}
