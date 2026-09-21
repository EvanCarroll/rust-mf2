//! P0.10 — hydration tolerance. Each page deliberately renders something
//! different on the server and on the client. They are excluded from the
//! zero-warning checks and have their own e2e script.

use leptos::prelude::*;

const SIDE: &str = if cfg!(feature = "ssr") { "SERVER" } else { "CLIENT" };

/// Text differences only: a static `&str`, a reactive text node and an
/// attribute. Expected: no warning; server text stays until the next update.
#[component]
pub fn TextMismatch() -> impl IntoView {
    let (n, set_n) = signal(0u32);
    let side: &'static str = SIDE;
    view! {
        <h1>"P0.10 text"</h1>
        <p id="static-diff">{side}</p>
        <p id="dynamic-diff">{move || {
            let mut s = String::from(side);
            s.push('-');
            s.push_str(&n.get().to_string());
            s
        }}</p>
        <p id="attr-diff" title=side>"attribute differs"</p>
        <button id="bump" on:click=move |_| set_n.update(|n| *n += 1)>"bump"</button>
    }
}

/// Structural difference: an element on the server, a text node on the client.
#[component]
pub fn StructMismatch() -> impl IntoView {
    let (n, set_n) = signal(0u32);
    let node = if cfg!(feature = "ssr") {
        view! { <b id="only-server">"element on the server"</b> }.into_any()
    } else {
        "text on the client".into_any()
    };
    view! {
        <h1>"P0.10 structure"</h1>
        <p id="struct-diff">{node}</p>
        <p id="after-count">{move || n.get()}</p>
        <button id="bump" on:click=move |_| set_n.update(|n| *n += 1)>"bump"</button>
    }
}

/// Same structural difference, but the client expects a `Tr` there: the probe's
/// glue must not panic (it logs and continues with a detached node).
#[component]
pub fn StructMismatchTr() -> impl IntoView {
    let (n, set_n) = signal(0u32);
    let node = if cfg!(feature = "ssr") {
        view! { <b id="only-server">"element on the server"</b> }.into_any()
    } else {
        crate::msg::WELCOME.into_any()
    };
    view! {
        <h1>"P0.10 structure (Tr)"</h1>
        <p id="struct-diff">{node}</p>
        <p id="after-count">{move || n.get()}</p>
        <button id="bump" on:click=move |_| set_n.update(|n| *n += 1)>"bump"</button>
    }
}
