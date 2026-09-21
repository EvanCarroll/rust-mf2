//! P0.11 browser harness: real `mf2-probe` `Tr` text nodes in a real DOM.
//! Built twice: default = strategy B (registry), `--features a` = strategy A
//! (`RenderEffect` per node). Driven by `../browser/run.mjs`.
//!
//! The churning list builds, mounts, unmounts and drops `rows` `<li>{tr}</li>`
//! views per round inside a per-round `Owner` (as a keyed list or a re-run
//! view closure would), synchronously; the page yields to the event loop
//! between batches so spawned effect tasks can finish, as between frames.
//! Row owners are children of one list owner that is cleaned after every
//! batch: `reactive_graph` prunes an owner's child list only on the parent's
//! cleanup, and the list's owner is where a real list would do that.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use leptos::html::{li, span};
use leptos::prelude::*;
use leptos::tachys::view::{Mountable, Render};
use mf2_probe::{live_bindings, set_catalog, tr};
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
    static OWNER: RefCell<Option<Owner>> = const { RefCell::new(None) };
    static LIST_OWNER: RefCell<Option<Owner>> = const { RefCell::new(None) };
    static LIVE_STATE: RefCell<Option<Box<dyn Mountable>>> = RefCell::new(None);
    static PREPARED: RefCell<Option<Vec<Box<str>>>> = const { RefCell::new(None) };
    static MESSAGES: Cell<u32> = const { Cell::new(1) };
}

fn catalog(tag: &str, n: u32) -> Vec<Box<str>> {
    (0..n)
        .map(|i| {
            let len = 10 + (i as usize * 7919) % 40;
            let mut s = String::with_capacity(len);
            s.push_str(tag);
            while s.len() < len {
                s.push_str(" word");
            }
            s.truncate(len);
            s.into_boxed_str()
        })
        .collect()
}

fn element(id: &str) -> leptos::web_sys::Element {
    document().get_element_by_id(id).expect("harness element")
}

/// "A" or "B".
#[wasm_bindgen]
pub fn strategy() -> String {
    if cfg!(feature = "a") { "A" } else { "B" }.to_owned()
}

/// Executor, root owner, `en` catalog of `messages` messages.
#[wasm_bindgen]
pub fn init(messages: u32) {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();
    let list = Owner::new();
    OWNER.with_borrow_mut(|o| *o = Some(owner));
    LIST_OWNER.with_borrow_mut(|o| *o = Some(list));
    MESSAGES.set(messages.max(1));
    set_catalog(catalog("en", messages));
}

/// Mounts `n` live `<span>{tr(i)}</span>` nodes under `#live`.
#[wasm_bindgen]
pub fn mount_live(n: u32) {
    let m = MESSAGES.get();
    let parent = element("live");
    let v: Vec<_> = (0..n).map(|i| span().child(tr(i % m))).collect();
    let mut st = v.build();
    st.mount(&parent, None);
    LIVE_STATE.with_borrow_mut(|s| *s = Some(Box::new(st)));
}

/// Mounts and unmounts `nodes` list rows under `#list`, `rows` per round.
#[wasm_bindgen]
pub fn churn(nodes: u32, rows: u32, offset: u32) {
    let m = MESSAGES.get();
    let parent = element("list");
    let rows = rows.max(1);
    let list = LIST_OWNER.with_borrow(|o| o.clone()).expect("init");
    for r in 0..nodes / rows {
        let row_owner = list.with(Owner::new);
        let base = offset + r * rows;
        let mut st = row_owner.with(|| {
            (0..rows)
                .map(|j| li().child(tr((base + j) % m)))
                .collect::<Vec<_>>()
                .build()
        });
        st.mount(&parent, None);
        st.unmount();
        drop(st);
        row_owner.cleanup();
    }
    list.cleanup();
}

/// Builds the other locale's catalog ahead of a timed switch (so the timing
/// covers only the update of the nodes, not building a catalog).
#[wasm_bindgen]
pub fn prepare(which: u32) {
    let c = catalog(if which % 2 == 0 { "en" } else { "pl" }, MESSAGES.get());
    PREPARED.with_borrow_mut(|p| *p = Some(c));
}

/// Installs the prepared catalog: strategy B walks its registry here;
/// strategy A notifies and its effects re-run in the following microtasks.
#[wasm_bindgen]
pub fn switch_prepared() {
    if let Some(c) = PREPARED.with_borrow_mut(Option::take) {
        set_catalog(c);
    }
}

/// Live heap bytes (counting allocator).
#[wasm_bindgen]
pub fn heap_live() -> f64 {
    LIVE.load(Relaxed) as f64
}

/// Allocations so far.
#[wasm_bindgen]
pub fn heap_allocs() -> f64 {
    ALLOCS.load(Relaxed) as f64
}

/// Live bindings reported by the library.
#[wasm_bindgen]
pub fn bindings() -> u32 {
    u32::try_from(live_bindings()).unwrap_or(u32::MAX)
}
