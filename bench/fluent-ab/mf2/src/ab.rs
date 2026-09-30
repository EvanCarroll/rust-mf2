//! The A/B's timing hooks, `mf2` side (bench/fluent-ab/README.md).
//!
//! Compiled only into the build the browser times (`ab-bench`), never into
//! the one whose size is measured. Its twin, `../../fluent/ab.rs`, has the
//! same exports doing the same work through `leptos-fluent`; `cargo xtask
//! fluent-ab` fills the message ids in from the workload, the same ones on
//! both sides.
//!
//! * `app` — the application, with a `performance.mark("ab-hydrated")` in
//!   the first animation frame after hydration;
//! * `ab_mount(n)` — `n` live translated text nodes, appended to the body,
//!   cycling over the workload's first simple messages;
//! * `ab_format(kind, iters)` — `iters` formats to a `String`: `0` a simple
//!   message, `1` one with one argument, `2` a plural select;
//! * `ab_switch(tag)` — the switch those nodes follow (it fetches the
//!   catalog, so the browser warms it first with `ab_preload`).

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;
use crate::tr;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn mark(name: &str);
}

/// The application, marking the first frame after it hydrated.
pub fn app() -> impl IntoView {
    Effect::new(|_| request_animation_frame(|| mark("ab-hydrated")));
    crate::app::App()
}

#[wasm_bindgen]
pub fn ab_mount(n: u32) {
    leptos::mount::mount_to_body(move || {
        let nodes = (0..n).map(node).collect_view();
        view! { <div id="ab-nodes">{nodes}</div> }
    });
}

fn node(i: u32) -> AnyView {
    match i % {{NODE_COUNT}} {
{{NODE_ARMS}}
    }
}

#[wasm_bindgen]
pub fn ab_format(kind: u32, iters: u32) -> usize {
    let mut bytes = 0;
    for i in 0..iters {
        let text = match kind {
            0 => tr!("{{SIMPLE}}").to_string(),
            1 => tr!("{{ONE}}", {{ONE_ARG}} = "Ada").to_string(),
            _ => tr!("{{SELECT}}", {{SELECT_ARG}} = i64::from(i % 7)).to_string(),
        };
        bytes += text.len();
    }
    bytes
}

#[wasm_bindgen]
pub fn ab_switch(tag: String) -> bool {
    leptos::task::spawn_local(async move {
        let _ = mf2::leptos::set_locale(&tag).await;
    });
    true
}

/// Fetches and validates `tag`'s catalog, so that the switch the browser
/// times does not wait on the network.
#[wasm_bindgen]
pub fn ab_preload(tag: String) {
    leptos::task::spawn_local(async move {
        let _ = mf2::leptos::preload_locale(&tag).await;
    });
}
