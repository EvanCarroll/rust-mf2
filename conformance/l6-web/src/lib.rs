//! Conformance layer L6 in the browser
//! (`plans/14-phase-6-work-order.md` A6b).
//!
//! The same call sites layer L6 renders on the server, on **one page**, so
//! that a real engine can be asked three things the server cannot answer:
//!
//! 1. does the page hydrate with **no** warning and **no** change of text?
//!    (P0.10: a text difference is silent, a structural one traps the wasm —
//!    so "nothing in the console" is the assertion, not a nicety);
//! 2. does a locale switch reach **every** node? The twin locale carries the
//!    same messages, so the switch rewrites all of them;
//! 3. does switching **back** restore the server's text exactly? That is the
//!    one comparison that catches a node the registry rewrote wrongly, or
//!    stopped tracking.
//!
//! What the browser never re-checks is `exp`: the server-rendered text *is*
//! `exp`, because layer L6 asserts that in Rust over these same cases. Here
//! the question is whether the client agrees with the server, which is a
//! different question and the only one a browser can answer.

// Generated code: the suite's own strings, escaped by the generator (a
// message may carry a bidi control, which rustc refuses in a literal).
#![allow(clippy::unreadable_literal, clippy::manual_string_new)]

use leptos::prelude::*;

mf2::include_generated!();

include!(concat!(env!("OUT_DIR"), "/shared.rs"));

include!(concat!(env!("OUT_DIR"), "/cases.rs"));

/// The locale the page is rendered in, and the twin it switches to.
pub const PAGE_LOCALE: &str = "en-US";
/// The twin: the same messages, a different tag and a different catalog.
pub const TWIN_LOCALE: &str = "en-GB";

/// Everything `leptos_mf2::install` needs, from the generated module.
#[must_use]
pub fn setup() -> leptos_mf2::Setup {
    leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}

/// One `<div>` per call site, in the order `CASES` lists them, each carrying
/// its message id so that a failure names a test rather than a position.
#[component]
pub fn Page() -> impl IntoView {
    let cases: Vec<_> = CASES
        .iter()
        .map(|(id, build)| {
            view! {
                <div class="case" data-id=*id>
                    {render(build())}
                </div>
            }
        })
        .collect();
    view! { <main id="cases">{cases}</main> }
}

/// A call site, as a view. The three shapes `tr!` expands to all render; the
/// renderer does not care which one it was given, which is the point.
fn render(case: Case) -> AnyView {
    match case {
        Case::Tr(t) => t.into_any(),
        Case::Args(t) => t.into_any(),
        Case::Dyn(t) => t.into_any(),
    }
}

/// The client: install, boot, hydrate — the ordinary entry point, so that
/// what this page exercises is what an application does.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(setup());
    leptos_mf2::hydrate_body(Page);
}

/// How many nodes follow the locale: what the check polls to know that
/// hydration has finished, and reads again after a switch.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    leptos_mf2::live_nodes()
}

/// Switches to `tag` and reports what happened, so that a failure in the
/// engine is a message rather than a silent non-event.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn mf2_set_locale(tag: String) -> Result<(), wasm_bindgen::JsValue> {
    leptos_mf2::set_locale(&tag)
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))
}
