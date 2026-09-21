use any_spawner::Executor;
use reactive_graph::{
    effect::RenderEffect,
    owner::Owner,
    signal::ArcTrigger,
    traits::{Notify, Track},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    time::Instant,
};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size(), Relaxed);
        LIVE.fetch_add(l.size(), Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn snap() -> (usize, usize, usize) {
    (ALLOCS.load(Relaxed), BYTES.load(Relaxed), LIVE.load(Relaxed))
}

fn main() {
    _ = Executor::init_futures_executor();
    let owner = Owner::new();
    owner.set();
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let trigger = ArcTrigger::new();
    let runs = std::sync::Arc::new(AtomicUsize::new(0));

    // warm up
    let mut effects: Vec<RenderEffect<(u32, String)>> = Vec::with_capacity(n);
    let before = snap();
    let t = Instant::now();
    for i in 0..n {
        let trigger = trigger.clone();
        let runs = runs.clone();
        effects.push(RenderEffect::new(move |prev: Option<(u32, String)>| {
            trigger.track();
            runs.fetch_add(1, Relaxed);
            match prev {
                Some(mut p) => { p.0 += 1; p }
                None => (i as u32, String::new()),
            }
        }));
    }
    let create = t.elapsed();
    let after = snap();
    println!("pointer width: {} bytes", std::mem::size_of::<usize>());
    println!("created {n} RenderEffects in {:?}", create);
    println!(
        "  per effect: {:.1} allocs, {:.0} bytes allocated, {:.0} bytes live",
        (after.0 - before.0) as f64 / n as f64,
        (after.1 - before.1) as f64 / n as f64,
        (after.2 - before.2) as f64 / n as f64
    );
    Executor::poll_local();
    let after_poll = snap();
    println!(
        "  after first poll of tasks: {:.0} bytes live per effect",
        (after_poll.2 - before.2) as f64 / n as f64
    );

    for round in 0..3 {
        let r0 = runs.load(Relaxed);
        let b = snap();
        let t = Instant::now();
        trigger.notify();
        let notify = t.elapsed();
        Executor::poll_local();
        let total = t.elapsed();
        let a = snap();
        println!(
            "round {round}: notify() {:?}, notify+rerun all {:?}, reruns {}, allocs during switch {:.1}/effect",
            notify, total, runs.load(Relaxed) - r0, (a.0 - b.0) as f64 / n as f64
        );
    }

    // churn: drop all effects without notifying, recreate, observe subscriber-set growth
    let b = snap();
    drop(effects);
    Executor::poll_local();
    let a = snap();
    println!(
        "after dropping all effects (no notify): {:.0} bytes still live per dropped effect",
        (a.2 as f64 - before.2 as f64) / n as f64
    );
    let _ = b;
    trigger.notify();
    Executor::poll_local();
    let a2 = snap();
    println!(
        "after a subsequent notify(): {:.0} bytes still live per dropped effect",
        (a2.2 as f64 - before.2 as f64) / n as f64
    );
}
