//! Conformance layer L7 in the browser
//! (`plans/01-conformance.md` §3; `plans/15-phase-7-work-order.md` A4).
//!
//! L6(b) server-renders every runtime-valid suite message on one page,
//! hydrates it, switches it to a twin locale and back. L7 asks the same of
//! the two other ways a page reaches a reader:
//!
//! * **islands** (columns `L7`, `L7d`): the server writes the page, and every
//!   call site sits in an island of its own behind the islands gate, so each
//!   one hydrates separately, against the catalog the gate waited for, and
//!   follows a live switch from inside its island;
//! * **client-only** (columns `L7c`, `L7cd`): an empty `<body>`, a published
//!   catalog index, and `mount_to_body` — nothing the server rendered, so
//!   the only thing to compare the page with is what the server *would*
//!   have rendered, which the `l7-page` binary writes beside it.
//!
//! One page per locale the suite uses, because a page is in one locale at a
//! time and the suite has four: the wasm carries four sets
//! (`conformance/l7-web/sets/*`) and installs the one its page names in
//! `<html data-l7-set>`.

use leptos::prelude::*;

/// The twin every set switches to and back from (`mf2-l7-set-build`).
pub const TWIN: &str = "en-GB";

/// The attribute of `<html>` that names the page's set, by its locale.
pub const SET_ATTR: &str = "data-l7-set";

/// One locale of the suite, as a page needs it.
pub struct Set {
    /// The set's locale: the page's, before any switch.
    pub locale: &'static str,
    /// The generated `Setup`.
    pub setup: fn() -> mf2::leptos::Setup,
    /// How many call sites the page holds.
    pub len: fn() -> usize,
    /// Call site `index`'s message id.
    pub id: fn(usize) -> Option<&'static str>,
    /// Call site `index`, as a view.
    pub view: fn(usize) -> AnyView,
    /// Every catalog of the set: the locale's and the twin's, each with its
    /// server-only table.
    #[cfg(feature = "ssr")]
    pub catalogs: &'static [(&'static str, &'static str, &'static [u8], &'static [u8])],
}

macro_rules! set {
    ($krate:ident) => {
        Set {
            locale: $krate::SOURCE_LOCALE,
            setup: $krate::setup,
            len: $krate::len,
            id: $krate::id,
            view: $krate::view,
            #[cfg(feature = "ssr")]
            catalogs: $krate::CATALOGS,
        }
    };
}

/// The four sets, one per locale the suite uses.
pub static SETS: [Set; 4] = [
    set!(mf2_l7_set_en_us),
    set!(mf2_l7_set_und),
    set!(mf2_l7_set_fr),
    set!(mf2_l7_set_ar),
];

/// The index into [`SETS`] of the set for `locale`.
#[must_use]
pub fn set_index(locale: &str) -> Option<usize> {
    SETS.iter().position(|s| s.locale == locale)
}

/// One call site, as an island of its own: the server writes it with its
/// props, and the client's walk hydrates it — after the gate, so against the
/// page's catalog. Not in the client-only build, which has no islands.
#[cfg(not(feature = "csr"))]
#[island]
fn L7Case(set: usize, index: usize) -> impl IntoView {
    match SETS.get(set) {
        Some(s) => (s.view)(index),
        None => ().into_any(),
    }
}

/// One `<div>` per call site of set `set`, each carrying its message id so
/// that a failure names a test rather than a position. With `islands`, each
/// call site is an island; without, it is a plain view.
#[component]
pub fn Cases(set: usize, islands: bool) -> impl IntoView {
    let Some(s) = SETS.get(set) else {
        return ().into_any();
    };
    let cases = (0..(s.len)())
        .map(|index| {
            let body = case(s, set, index, islands);
            view! { <div class="case" data-id=(s.id)(index).unwrap_or_default()>{body}</div> }
        })
        .collect_view();
    view! { <main id="cases">{cases}</main> }.into_any()
}

/// Call site `index` of `s` (which is `SETS[set]`), in an island or not.
fn case(s: &Set, set: usize, index: usize, islands: bool) -> AnyView {
    #[cfg(not(feature = "csr"))]
    if islands {
        return view! { <L7Case set index /> }.into_any();
    }
    let _ = (set, islands);
    (s.view)(index)
}

/// The set the page names.
#[cfg(any(feature = "hydrate", feature = "csr"))]
fn page_set() -> Option<usize> {
    let tag = document().document_element()?.get_attribute(SET_ATTR)?;
    set_index(&tag)
}

/// The islands client: install the page's set and boot. Leptos' island
/// script calls this, then walks the islands — the gate first.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    let Some(set) = page_set() else {
        leptos::logging::error!("l7: the page names no set");
        return;
    };
    mf2::leptos::install((SETS[set].setup)());
    mf2::leptos::hydrate_islands();
}

// The island `IslandsGate` renders.
mf2::leptos::islands_gate!();

/// The client-only entry point: install the page's set and mount it once its
/// catalog, chosen from the published index, is installed.
#[cfg(feature = "csr")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start() {
    console_error_panic_hook::set_once();
    let Some(set) = page_set() else {
        leptos::logging::error!("l7: the page names no set");
        return;
    };
    mf2::leptos::install((SETS[set].setup)());
    mf2::leptos::mount_to_body(move || view! { <Cases set islands=false /> });
}

/// How many nodes follow the locale: what the check polls to know that the
/// page has hydrated or mounted, and reads again after a switch.
#[cfg(any(feature = "hydrate", feature = "csr"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    mf2::leptos::live_nodes()
}

/// Switches to `tag` and reports what happened.
#[cfg(any(feature = "hydrate", feature = "csr"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn mf2_set_locale(tag: String) -> Result<(), wasm_bindgen::JsValue> {
    mf2::leptos::set_locale(&tag)
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))
}
