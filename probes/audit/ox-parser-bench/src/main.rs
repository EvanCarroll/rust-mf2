use ox_mf2_parser::{build_semantic_model, parse_source, validate_semantics, ParseOptions, SourceFileInput, SourceStore};
use serde_json::Value;
use std::{alloc::{GlobalAlloc, Layout, System}, fs, hint::black_box, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering::Relaxed}, time::Instant};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { ALLOCS.fetch_add(1, Relaxed); BYTES.fetch_add(l.size() as u64, Relaxed); unsafe { System.alloc(l) } }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p, l) } }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 { ALLOCS.fetch_add(1, Relaxed); BYTES.fetch_add(n as u64, Relaxed); unsafe { System.realloc(p, l, n) } }
}
#[global_allocator] static A: Counting = Counting;

fn walk(d: &Path, out: &mut Vec<PathBuf>) { for e in fs::read_dir(d).unwrap() { let p = e.unwrap().path(); if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "json") { out.push(p) } } }

// deterministic reference-workload-shaped corpus: 79% simple, 14.7/5/1.1/0.2% with 1-4 vars, 0.9% plural select
fn workload(n: usize) -> Vec<String> {
    let words = ["Send","Cancel","Your","profile","settings","message","could","not","be","saved","right","now","please","try","again","later","members","online","welcome","back"];
    let mut s: u64 = 0x9E3779B97F4A7C15; let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; s };
    (0..n).map(|i| {
        let r = (next() % 1000) as usize;
        let len = 2 + (next() % 7) as usize;
        let text = |next: &mut dyn FnMut() -> u64, k: usize| (0..k).map(|_| words[(next() % words.len() as u64) as usize]).collect::<Vec<_>>().join(" ");
        if r < 9 { format!(".input {{$count :integer}}\n.match $count\none {{{{{{$count}} {} }}}}\n* {{{{{{$count}} {} }}}}", text(&mut next, 2), text(&mut next, 2)) }
        else if r < 9 + 210 { let vars = if r < 9+147 {1} else if r < 9+197 {2} else if r < 9+208 {3} else {4};
            let mut m = text(&mut next, len); for v in 0..vars { m.push_str(&format!(" {{$arg{v}}} ")); m.push_str(&text(&mut next, 2)); } m }
        else { let _ = i; text(&mut next, len) }
    }).collect()
}

fn bench(name: &str, msgs: &[String], full: bool) {
    let bytes: usize = msgs.iter().map(|m| m.len()).sum();
    let run = |msgs: &[String]| { let mut errs = 0usize; for src in msgs {
        let mut sources = SourceStore::new();
        let id = sources.add(SourceFileInput { source: src, ..Default::default() });
        let res = parse_source(&sources, id, ParseOptions::default()).unwrap();
        if full && res.diagnostics.is_empty() { if let Ok(ds) = build_semantic_model(&sources, &res).and_then(|m| validate_semantics(&m)) { errs += ds.len(); } } else { errs += res.diagnostics.len(); }
        black_box(&res); } errs };
    black_box(run(msgs)); // warm
    let (a0, b0) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
    black_box(run(msgs));
    let (a1, b1) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
    let iters = 200; let t = Instant::now(); for _ in 0..iters { black_box(run(msgs)); } let dt = t.elapsed();
    let per = dt.as_nanos() as f64 / (iters * msgs.len()) as f64;
    println!("{name:<44} {:>5} msgs {:>7} B | {:>8.0} ns/msg | {:>6.1} MB/s | {:>5.1} allocs/msg | {:>6.0} alloc B/msg",
        msgs.len(), bytes, per, (bytes * iters) as f64 / dt.as_secs_f64() / 1e6, (a1-a0) as f64 / msgs.len() as f64, (b1-b0) as f64 / msgs.len() as f64);
}

fn main() {
    let root = std::env::args().nth(1).unwrap();
    let mut files = vec![]; walk(Path::new(&root), &mut files); files.sort();
    let mut suite = vec![];
    for f in files { let v: Value = serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        for t in v["tests"].as_array().unwrap() { if let Some(s) = t.get("src").and_then(Value::as_str) { suite.push(s.to_owned()); } } }
    let wl = workload(1600);
    let simple: Vec<String> = wl.iter().filter(|m| !m.contains('{')).cloned().collect();
    bench("suite: parse -> CST", &suite, false);
    bench("suite: parse + semantic + validate", &suite, true);
    bench("workload(1600): parse -> CST", &wl, false);
    bench("workload(1600): parse + semantic + validate", &wl, true);
    bench("workload simple-only: parse+semantic+validate", &simple, true);
}
