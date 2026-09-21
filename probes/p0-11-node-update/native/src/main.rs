//! P0.11 native model: how translated nodes follow a locale switch.
//!
//! * **A** — one `RenderEffect` per node, tracking one global `ArcTrigger`
//!   (the planning prototype; `mf2-probe --features effect`).
//! * **B** — a library-owned registry: one slab slot `{node, MsgId}` per node,
//!   freed on drop; a switch walks the slab (`mf2-probe` default).
//!
//! Real `reactive_graph` 0.2 (Leptos 0.8) and its executor; the DOM is a fake
//! (a slab of `String`s), identical for both strategies, since tachys cannot
//! create nodes natively. Each strategy runs in its own process so the
//! counting allocator sees only its own heap.
//!
//! Run (one command, both strategies):
//! `cargo run --release -- both`

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use any_spawner::Executor;
use clap::{Parser, ValueEnum};
use reactive_graph::effect::RenderEffect;
use reactive_graph::owner::Owner;
use reactive_graph::signal::ArcTrigger;
use reactive_graph::traits::{Notify, Track};

// ---------- counting allocator ----------

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

fn live() -> usize {
    LIVE.load(Relaxed)
}
fn allocs() -> usize {
    ALLOCS.load(Relaxed)
}

// ---------- fake DOM (same for both strategies) ----------

#[derive(Default)]
struct Dom {
    texts: Vec<String>,
    free: Vec<u32>,
}

thread_local! {
    static DOM: RefCell<Dom> = RefCell::new(Dom::default());
    static CATALOG: RefCell<Rc<Vec<Box<str>>>> = RefCell::new(Rc::new(Vec::new()));
    static TRIGGER: ArcTrigger = ArcTrigger::new();
}

fn create_text(s: &str) -> u32 {
    DOM.with_borrow_mut(|d| {
        if let Some(n) = d.free.pop() {
            let t = &mut d.texts[n as usize];
            t.clear();
            t.push_str(s);
            n
        } else {
            d.texts.push(s.to_owned());
            (d.texts.len() - 1) as u32
        }
    })
}

fn set_text(n: u32, s: &str) {
    DOM.with_borrow_mut(|d| {
        let t = &mut d.texts[n as usize];
        t.clear();
        t.push_str(s);
    });
}

fn remove_text(n: u32) {
    DOM.with_borrow_mut(|d| d.free.push(n));
}

fn get_text(n: u32) -> String {
    DOM.with_borrow(|d| d.texts[n as usize].clone())
}

fn with_msg<R>(id: u32, f: impl FnOnce(&str) -> R) -> R {
    let cat = CATALOG.with_borrow(Rc::clone);
    f(cat.get(id as usize).map_or("", |s| s))
}

// ---------- strategy A: RenderEffect per node ----------

struct NodeA {
    node: u32,
    _fx: RenderEffect<()>,
}

fn make_a(id: u32) -> NodeA {
    let node = with_msg(id, create_text);
    let trigger = TRIGGER.with(Clone::clone);
    let fx = RenderEffect::new(move |prev: Option<()>| {
        trigger.track();
        if prev.is_some() {
            with_msg(id, |s| set_text(node, s));
        }
    });
    NodeA { node, _fx: fx }
}

impl Drop for NodeA {
    fn drop(&mut self) {
        remove_text(self.node);
    }
}

// ---------- strategy B: library-owned registry ----------

enum Slot {
    Used { node: u32, id: u32 },
    Free(Option<u32>),
}

#[derive(Default)]
struct Registry {
    slots: Vec<Slot>,
    free: Option<u32>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

struct NodeB {
    slot: u32,
}

fn make_b(id: u32) -> NodeB {
    let node = with_msg(id, create_text);
    let slot = REGISTRY.with_borrow_mut(|r| {
        let used = Slot::Used { node, id };
        if let Some(i) = r.free {
            let next = match r.slots[i as usize] {
                Slot::Free(n) => n,
                Slot::Used { .. } => None,
            };
            r.slots[i as usize] = used;
            r.free = next;
            i
        } else {
            r.slots.push(used);
            (r.slots.len() - 1) as u32
        }
    });
    NodeB { slot }
}

impl Drop for NodeB {
    fn drop(&mut self) {
        let node = REGISTRY.with_borrow_mut(|r| {
            let old = std::mem::replace(&mut r.slots[self.slot as usize], Slot::Free(r.free));
            r.free = Some(self.slot);
            match old {
                Slot::Used { node, .. } => Some(node),
                Slot::Free(_) => None,
            }
        });
        if let Some(n) = node {
            remove_text(n);
        }
    }
}

fn refresh_b() {
    REGISTRY.with_borrow(|r| {
        for s in &r.slots {
            if let Slot::Used { node, id } = s {
                with_msg(*id, |t| set_text(*node, t));
            }
        }
    });
}

// ---------- the experiment ----------

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Which {
    A,
    B,
    Both,
}

#[derive(Parser)]
#[command(about = "P0.11: RenderEffect per node (A) vs registry (B), native model")]
struct Cli {
    /// Strategy; `both` runs A and B in separate processes.
    #[arg(value_enum, default_value = "both")]
    which: Which,
    /// Live translated nodes.
    #[arg(long, default_value_t = 2000)]
    live: u32,
    /// Nodes mounted and unmounted by the churning list.
    #[arg(long, default_value_t = 100_000)]
    churn: u32,
    /// Rows the list shows at a time (one churn round replaces all of them).
    #[arg(long, default_value_t = 50)]
    rows: u32,
    /// Messages per catalog.
    #[arg(long, default_value_t = 1600)]
    messages: u32,
    /// Timed locale switches (median reported).
    #[arg(long, default_value_t = 21)]
    switches: usize,
}

fn catalog(tag: &str, n: u32) -> Rc<Vec<Box<str>>> {
    // ~27 B mean like plans/06 §2; content irrelevant.
    Rc::new(
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
            .collect(),
    )
}

enum Node {
    A(#[allow(dead_code)] NodeA),
    B(#[allow(dead_code)] NodeB),
}

fn make(which: Which, id: u32) -> Node {
    match which {
        Which::A => Node::A(make_a(id)),
        _ => Node::B(make_b(id)),
    }
}

fn switch(which: Which, cat: &Rc<Vec<Box<str>>>) -> Duration {
    let t = Instant::now();
    CATALOG.with_borrow_mut(|c| *c = Rc::clone(cat));
    if which == Which::B {
        refresh_b();
    }
    TRIGGER.with(Notify::notify);
    Executor::poll_local();
    t.elapsed()
}

fn median(v: &mut [Duration]) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn run(which: Which, cli: &Cli) {
    let name = if which == Which::A { "A (RenderEffect per node)" } else { "B (registry)" };
    Executor::init_futures_executor().expect("executor");
    let owner = Owner::new();
    owner.set();
    let en = catalog("en", cli.messages);
    let pl = catalog("pl", cli.messages);
    CATALOG.with_borrow_mut(|c| *c = Rc::clone(&en));
    // Warm the fake DOM and registry capacity so that only per-node costs
    // show up below (both strategies get the same warm-up).
    {
        let warm: Vec<Node> = (0..cli.live + cli.rows).map(|i| make(which, i % cli.messages)).collect();
        drop(warm);
        TRIGGER.with(Notify::notify);
        Executor::poll_local();
    }

    println!("## {name}");
    let (l0, a0) = (live(), allocs());
    let t = Instant::now();
    let live_nodes: Vec<Node> = (0..cli.live).map(|i| make(which, i % cli.messages)).collect();
    let create = t.elapsed();
    Executor::poll_local();
    let (l1, a1) = (live(), allocs());
    println!(
        "live nodes: {} created in {:?}; per node: {:.0} B live heap, {:.1} allocations",
        cli.live,
        create,
        (l1 - l0) as f64 / f64::from(cli.live),
        (a1 - a0) as f64 / f64::from(cli.live)
    );

    let mut times: Vec<Duration> = (0..cli.switches)
        .map(|k| switch(which, if k % 2 == 0 { &pl } else { &en }))
        .collect();
    let (min, max) = (*times.iter().min().expect("n"), *times.iter().max().expect("n"));
    println!(
        "switch with {} live nodes: median {:?} (min {:?}, max {:?}, n={})",
        cli.live,
        median(&mut times),
        min,
        max,
        cli.switches
    );
    // Make sure the last switch really updated the nodes.
    let expect = CATALOG.with_borrow(|c| c[1].to_string());
    let node1 = match &live_nodes[1] {
        Node::A(n) => n.node,
        Node::B(n) => REGISTRY.with_borrow(|r| match r.slots[n.slot as usize] {
            Slot::Used { node, .. } => node,
            Slot::Free(_) => u32::MAX,
        }),
    };
    assert_eq!(get_text(node1), expect, "switch did not update node 1");

    // Churn: a list whose rows are replaced, `rows` at a time, with no
    // locale switch in between (the common case: scrolling a log).
    // Row owners are children of one list owner, cleaned after every batch of
    // ten rounds (reactive_graph prunes a child list only on the parent's
    // cleanup; a real list's owner does that).
    let list_owner = Owner::new();
    let before = live();
    let rounds = cli.churn / cli.rows;
    let mut samples = Vec::new();
    let t = Instant::now();
    for r in 0..rounds {
        let row_owner = list_owner.with(Owner::new);
        let rows: Vec<Node> =
            row_owner.with(|| (0..cli.rows).map(|j| make(which, (r * cli.rows + j) % cli.messages)).collect());
        Executor::poll_local();
        drop(rows);
        row_owner.cleanup();
        drop(row_owner);
        if r % 10 == 9 {
            list_owner.cleanup();
        }
        Executor::poll_local();
        let done = (r + 1) * cli.rows;
        if done % (cli.churn / 10).max(1) == 0 {
            samples.push((done, live() as i64 - before as i64));
        }
    }
    let churn_time = t.elapsed();
    let grown = live() as i64 - before as i64;
    println!(
        "churn: {} nodes mounted+unmounted in {:?}; live heap growth {} B ({:.1} B per churned node)",
        rounds * cli.rows,
        churn_time,
        grown,
        grown as f64 / f64::from(rounds * cli.rows)
    );
    let s: Vec<String> = samples.iter().map(|(n, b)| format!("{}k:{:+}", n / 1000, b)).collect();
    println!("  heap growth by churned nodes: {}", s.join(" "));

    let mut after: Vec<Duration> = (0..cli.switches)
        .map(|k| switch(which, if k % 2 == 0 { &pl } else { &en }))
        .collect();
    let first = after[0];
    println!(
        "switch after churn: first {:?}, median {:?}; live heap growth after it: {} B",
        first,
        median(&mut after),
        live() as i64 - before as i64
    );
    drop(live_nodes);
}

fn main() {
    let cli = Cli::parse();
    match cli.which {
        Which::Both => {
            let exe = std::env::current_exe().expect("exe");
            let pass: Vec<String> = std::env::args().skip(1).filter(|a| a != "both").collect();
            for w in ["a", "b"] {
                let st = Command::new(&exe).arg(w).args(&pass).status().expect("spawn");
                assert!(st.success());
            }
        }
        w => {
            println!(
                "pointer width {} B; live {}, churn {} ({} rows/round), {} messages",
                size_of::<usize>(),
                cli.live,
                cli.churn,
                cli.rows,
                cli.messages
            );
            run(w, &cli);
        }
    }
}
