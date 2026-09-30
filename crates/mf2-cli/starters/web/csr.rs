use leptos::prelude::*;
use leptos_meta::{Title, provide_meta_context};
use mf2::leptos::LocaleSwitcher;

mf2::include_generated!();

#[component]
fn App() -> impl IntoView {
    provide_meta_context();
    let count = RwSignal::new(1);
    view! {
        <Title text=tr!("app-title") />
        <header>
            <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply") />
        </header>
        <main>
            <h1>{tr!("greeting", name = "Ada")}</h1>
            <p role="status">{tr!("visits", count = count)}</p>
            <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
        </main>
    }
}

/// The language is the one the reader chose last time, else the browser's
/// best match, else English. The page mounts once its catalog is in.
fn main() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::mount_to_body(App);
}
