//! The lazy route (a wasm chunk of its own under `--split`) and the
//! not-found text. Declared before the include.

use crate::prelude::*;
use leptos::prelude::*;
use leptos_router::{LazyRoute, lazy_route};

pub struct Visits;

#[lazy_route]
impl LazyRoute for Visits {
    fn data() -> Self {
        Visits
    }

    fn view(this: Self) -> AnyView {
        let Visits = this;
        let count = RwSignal::new(1);
        view! {
            <p role="status">{tr!("visits", count = count)}</p>
            <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
        }
        .into_any()
    }
}

pub fn not_found() -> impl IntoView {
    view! { <p>{tr!("not-found")}</p> }
}
