//! The conversions under churn (Phase 7 A5), natively.
//!
//! P0.11's leak: a dropped effect stays in the subscriber set of every
//! source it read until that source next fires. The locale trigger fires only
//! on a switch, so an effect that read a `TextProp`, a `Signal<String>` or
//! `to_string()` — a row in a list that churns — used to leave itself behind
//! until the next switch. The browser measurement, every row shape included,
//! is `cargo xtask churn` (`bench/churn`); this is the part that needs no
//! DOM, on every push: the heap stays flat, the plain `track()` control does
//! not, and a live consumer still follows every switch.
//!
//! The client half, so it builds with `csr` natively:
//! `cargo test -p leptos-mf2 --no-default-features --features csr --test churn`.
//! Under `ssr` (the workspace build) there is no trigger, and nothing here.
#![cfg(not(feature = "ssr"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use any_spawner::Executor;
use leptos::prelude::*;
use leptos::text_prop::TextProp;
use leptos_mf2::{Setup, changed, install, set_active, tr};
use mf2::{Compiled, Dir, Function, Registry, functions};

/// This thread's live heap, so that the test harness's own threads do not
/// show up in it.
struct Counting;

std::thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

fn add(bytes: usize, sign: isize) {
    let bytes = isize::try_from(bytes).unwrap_or(isize::MAX);
    let _ = LIVE.try_with(|live| live.set(live.get() + sign * bytes));
}

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        add(l.size(), 1);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        add(l.size(), -1);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        add(new, 1);
        add(l.size(), -1);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live() -> isize {
    LIVE.with(Cell::get)
}

static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &functions::STRING)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr)];

const ROWS: usize = 50;
const WARM_UP: usize = 2_000;
const CHURN: usize = 20_000;

/// What 20,000 churned rows may leave behind: less than a byte a row. The
/// leak this guards against was ≈ 70 B a row on wasm32, more natively.
const FLAT: isize = 16_384;

/// Builds and drops `n` consumers, `ROWS` per round under a round owner —
/// a keyed list's shape — and lets the executor finish their tasks.
fn churn(list: &Owner, n: usize, consumer: &dyn Fn() -> RenderEffect<()>) {
    for _ in 0..n / ROWS {
        let round = list.with(Owner::new);
        let effects: Vec<RenderEffect<()>> = round.with(|| (0..ROWS).map(|_| consumer()).collect());
        drop(effects);
        round.cleanup();
        Executor::poll_local();
    }
    list.cleanup();
    Executor::poll_local();
}

/// The heap after `CHURN` rows, less the heap after the warm-up.
fn growth(consumer: &dyn Fn() -> RenderEffect<()>) -> isize {
    let list = Owner::new();
    churn(&list, WARM_UP, consumer);
    let before = live();
    churn(&list, CHURN, consumer);
    live() - before
}

/// A live consumer counts its runs: one at creation, one per switch.
fn follows(consumer: impl Fn() + Send + Sync + 'static) -> usize {
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    let effect = RenderEffect::new(move |_| {
        consumer();
        counted.fetch_add(1, Relaxed);
    });
    for _ in 0..3 {
        changed().notify();
        Executor::poll_local();
    }
    drop(effect);
    runs.load(Relaxed)
}

#[test]
fn conversions_leave_nothing_behind() {
    let _ = Executor::init_futures_executor();
    install(Setup::new(
        &REGISTRY,
        &mf2::host_std::HOST,
        0,
        "en",
        LOCALES,
    ));
    set_active(Arc::new(
        mf2::compile_str("A row", "en")
            .expect("the message compiles")
            .catalog,
    ));
    let root = Owner::new();
    root.set();
    let row = tr(Compiled::ID);

    let text_prop = || {
        let text = TextProp::from(row);
        RenderEffect::new(move |_| {
            let _ = text.get();
        })
    };
    let signal = || {
        let text: Signal<String> = row.into();
        RenderEffect::new(move |_| {
            let _ = text.get();
        })
    };
    let to_string = || {
        RenderEffect::new(move |_| {
            let _ = row.to_string();
        })
    };
    // The control: what the conversions did before A5. If this stops
    // growing, `reactive_graph` unsubscribes dropped effects itself and
    // `track_locale` has become unnecessary.
    let plain_track = || {
        RenderEffect::new(move |_| {
            changed().track();
        })
    };

    for (name, consumer) in [
        ("TextProp", &text_prop as &dyn Fn() -> RenderEffect<()>),
        ("Signal<String>", &signal),
        ("to_string()", &to_string),
    ] {
        let grown = growth(consumer);
        assert!(
            grown <= FLAT,
            "{name}: {CHURN} churned rows left {grown} B behind"
        );
    }
    let control = growth(&plain_track);
    assert!(
        control > isize::try_from(CHURN * 16).unwrap_or(isize::MAX),
        "the plain-track control left only {control} B behind {CHURN} rows: \
         reactive_graph now cleans up dropped subscribers itself"
    );

    // Unsubscribing at cleanup must not unsubscribe a consumer that lives
    // on: it re-subscribes on every run, so it follows every switch.
    assert_eq!(follows(move || drop(TextProp::from(row).get())), 4);
    assert_eq!(
        follows(move || {
            let text: Signal<String> = row.into();
            drop(text.get());
        }),
        4
    );
    assert_eq!(follows(move || drop(row.to_string())), 4);
}
