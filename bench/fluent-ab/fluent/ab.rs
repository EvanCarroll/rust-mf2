//! The A/B's timing hooks, `leptos-fluent` side (bench/fluent-ab/README.md).
//!
//! Compiled only into the build the browser times (`ab-bench`), never into
//! the one whose size is measured. Its twin, `../mf2/src/ab.rs`, has the same
//! exports doing the same work through `mf2`; `cargo xtask fluent-ab` fills
//! the message ids in from the workload, the same ones on both sides.
//!
//! * `app` — the application, with a `performance.mark("ab-hydrated")` in
//!   the first animation frame after hydration;
//! * `ab_mount(n)` — `n` live translated text nodes, appended to the body
//!   under the library's own provider, cycling over the workload's first
//!   simple messages;
//! * `ab_format(kind, iters)` — `iters` formats to a `String`: `0` a simple
//!   message, `1` one with one argument, `2` a plural select;
//! * `ab_switch(tag)` — the switch those nodes follow.

use std::cell::Cell;

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr, tr};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn mark(name: &str);
}

thread_local! {
    /// The context `ab_mount` rendered under.
    static I18N: Cell<Option<I18n>> = const { Cell::new(None) };
}

/// The application, marking the first frame after it hydrated.
pub fn app() -> impl IntoView {
    Effect::new(|_| request_animation_frame(|| mark("ab-hydrated")));
    crate::app::App()
}

#[wasm_bindgen]
pub fn ab_mount(n: u32) {
    leptos::mount::mount_to_body(move || {
        view! {
            <crate::support::I18nProvider>
                <Nodes n=n />
            </crate::support::I18nProvider>
        }
    });
}

#[component]
fn Nodes(n: u32) -> impl IntoView {
    I18N.set(Some(expect_context::<I18n>()));
    let nodes = (0..n).map(node).collect_view();
    view! { <div id="ab-nodes">{nodes}</div> }
}

fn node(i: u32) -> AnyView {
    match i % {{NODE_COUNT}} {
{{NODE_ARMS}}
    }
}

#[wasm_bindgen]
pub fn ab_format(kind: u32, iters: u32) -> usize {
    let Some(i18n) = I18N.get() else {
        return 0;
    };
    let mut bytes = 0;
    for i in 0..iters {
        let text = match kind {
            0 => tr!(i18n, "{{SIMPLE}}"),
            1 => tr!(i18n, "{{ONE}}", { "{{ONE_ARG}}" => "Ada" }),
            _ => tr!(i18n, "{{SELECT}}", { "{{SELECT_ARG}}" => i64::from(i % 7) }),
        };
        bytes += text.len();
    }
    bytes
}

#[wasm_bindgen]
pub fn ab_switch(tag: &str) -> bool {
    let Some(i18n) = I18N.get() else {
        return false;
    };
    let Some(language) = i18n.languages.iter().find(|l| l.id == tag) else {
        return false;
    };
    i18n.language.set(language);
    true
}

/// Nothing to fetch on this side: every locale is in the wasm.
#[wasm_bindgen]
pub fn ab_preload(_tag: &str) {}
