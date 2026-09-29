//! The churn harness (`plans/15-phase-7-work-order.md` A5): P0.11's
//! churning list, on `mf2::leptos` itself rather than the probe's glue.
//!
//! P0.11 settled D7 on the node registry because an effect per node leaks
//! under churn — a dropped `RenderEffect` stays in the locale trigger's
//! subscriber set until the trigger next fires, and a locale switch is rare.
//! The registry is flat. What P0.11 left open is the **conversions**:
//! `TextProp`, `Signal<String>` and `to_string()` under an observer
//! subscribe the *consumer's* effect to the same trigger, and a consumer in a
//! churning row is dropped with it. This harness measures each row shape:
//!
//! | Variant | Row | Follows a switch through |
//! |---|---|---|
//! | `text` | `<li>{tr!("row")}</li>` | the registry |
//! | `attr` | the same with `title=tr!("row-title")` | the registry (A3's attribute fix) |
//! | `args` | `<li>{tr!("row-n", n = n)}</li>`, `n` a signal | the registry, plus the node's argument effect |
//! | `textprop` | a component taking `#[prop(into)] TextProp` | the consumer's effect, subscribed by the conversion |
//! | `signal` | a component taking `#[prop(into)] Signal<String>` | the same |
//! | `to-string` | `<li>{move \|\| tr!("row").to_string()}</li>` | the same, subscribed by `to_string()` |
//! | `oco` | a component taking `#[prop(into)] Oco<'static, str>` | nothing: a value (the control) |
//!
//! Rows are built, mounted, unmounted and dropped `rows` at a time, each
//! round under its own `Owner`, children of one list owner cleaned after
//! every batch — as a keyed list or a re-run view closure does. The page
//! yields to the event loop between batches, as between frames, so that the
//! executor finishes the tasks of dropped effects. A counting allocator
//! reports the live heap. Driven by `tools/e2e/checks/churn.mjs`; built and
//! run by `cargo xtask churn`.
//!
//! A bench, not a client-path crate: it panics on a missing element and
//! holds the `unsafe` a `GlobalAlloc` needs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use churn_i18n::tr;
use leptos::oco::Oco;
use leptos::prelude::*;
use leptos::tachys::view::{Mountable, Render};
use leptos::text_prop::TextProp;
use wasm_bindgen::prelude::*;

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        LIVE.fetch_add(l.size(), Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        LIVE.fetch_add(new, Relaxed);
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

thread_local! {
    static ROOT: RefCell<Option<Owner>> = const { RefCell::new(None) };
    static LIST: RefCell<Option<Owner>> = const { RefCell::new(None) };
    static LIVE_VIEWS: RefCell<Vec<Box<dyn Mountable>>> = const { RefCell::new(Vec::new()) };
    static COUNT: Cell<Option<RwSignal<i32>>> = const { Cell::new(None) };
}

/// A row that holds its text as a `TextProp`, as a component prop does.
#[component]
fn PropRow(#[prop(into)] text: TextProp) -> impl IntoView {
    view! { <li>{move || text.get()}</li> }
}

/// …as a `Signal<String>`.
#[component]
fn SignalRow(#[prop(into)] text: Signal<String>) -> impl IntoView {
    view! { <li>{text}</li> }
}

/// …as an `Oco`: the text now, followed by nothing.
#[component]
fn OcoRow(#[prop(into)] text: Oco<'static, str>) -> impl IntoView {
    view! { <li>{text}</li> }
}

/// The variant names the check passes.
const VARIANTS: [&str; 7] = [
    "text",
    "attr",
    "args",
    "textprop",
    "signal",
    "to-string",
    "oco",
];

/// Every variant, in the order the check reports them.
#[wasm_bindgen]
pub fn variants() -> Vec<String> {
    VARIANTS.iter().map(|v| (*v).to_owned()).collect()
}

fn row(variant: &str) -> AnyView {
    match variant {
        "text" => view! { <li>{tr!("row")}</li> }.into_any(),
        "attr" => view! { <li title=tr!("row-title")>{tr!("row")}</li> }.into_any(),
        "args" => {
            let n = COUNT.get().expect("booted");
            view! { <li>{tr!("row-n", n = n)}</li> }.into_any()
        }
        "textprop" => view! { <PropRow text=tr!("row") /> }.into_any(),
        "signal" => view! { <SignalRow text=tr!("row") /> }.into_any(),
        "to-string" => view! { <li>{move || tr!("row").to_string()}</li> }.into_any(),
        "oco" => view! { <OcoRow text=tr!("row") /> }.into_any(),
        other => panic!("unknown variant {other}"),
    }
}

fn element(id: &str) -> web_sys::Element {
    document().get_element_by_id(id).expect("harness element")
}

/// Installs the generated setup, boots as a client-only application does
/// (index, locale, catalog), and sets up the executor and the owners.
#[wasm_bindgen]
pub async fn boot() -> Result<String, JsValue> {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    mf2::leptos::install(churn_i18n::setup());
    mf2::leptos::load_client_catalog()
        .await
        .map_err(|_| JsValue::from_str("the boot failed"))?;
    let root = Owner::new();
    root.set();
    let list = Owner::new();
    COUNT.set(Some(RwSignal::new(1)));
    ROOT.with_borrow_mut(|o| *o = Some(root));
    LIST.with_borrow_mut(|o| *o = Some(list));
    Ok(mf2::leptos::active()
        .map(|c| c.locale().to_owned())
        .unwrap_or_default())
}

/// Mounts `n` live rows of `variant` under `#live`; they stay.
#[wasm_bindgen]
pub fn mount_live(n: u32, variant: &str) {
    let parent = element("live");
    let rows: Vec<AnyView> = (0..n).map(|_| row(variant)).collect();
    let mut state = rows.build();
    state.mount(&parent, None);
    LIVE_VIEWS.with_borrow_mut(|v| v.push(Box::new(state)));
}

/// Unmounts and drops every live row.
#[wasm_bindgen]
pub fn clear_live() {
    for mut state in LIVE_VIEWS.with_borrow_mut(std::mem::take) {
        state.unmount();
    }
}

/// Builds, mounts, unmounts and drops `nodes` rows of `variant` under
/// `#list`, `rows` per round, each round under its own owner.
#[wasm_bindgen]
pub fn churn(nodes: u32, rows: u32, variant: &str) {
    let parent = element("list");
    let rows = rows.max(1);
    let list = LIST.with_borrow(Clone::clone).expect("booted");
    for _ in 0..nodes / rows {
        let round = list.with(Owner::new);
        let mut state = round.with(|| (0..rows).map(|_| row(variant)).collect::<Vec<_>>().build());
        state.mount(&parent, None);
        state.unmount();
        drop(state);
        round.cleanup();
    }
    list.cleanup();
}

/// A live switch: fetch, swap, relocalize the registry, notify.
#[wasm_bindgen]
pub async fn switch_locale(tag: String) -> Result<(), JsValue> {
    mf2::leptos::set_locale(&tag)
        .await
        .map_err(|_| JsValue::from_str("the switch failed"))
}

/// Fires the locale trigger alone — the part of a switch whose cost grows
/// with the trigger's subscriber set, dead subscribers included.
#[wasm_bindgen]
pub fn notify() {
    mf2::leptos::changed().notify();
}

/// Live heap bytes (the counting allocator).
#[wasm_bindgen]
pub fn heap_live() -> f64 {
    LIVE.load(Relaxed) as f64
}

/// Allocations so far.
#[wasm_bindgen]
pub fn heap_allocs() -> f64 {
    ALLOCS.load(Relaxed) as f64
}

/// Live registry slots.
#[wasm_bindgen]
pub fn live_nodes() -> u32 {
    u32::try_from(mf2::leptos::live_nodes()).unwrap_or(u32::MAX)
}
