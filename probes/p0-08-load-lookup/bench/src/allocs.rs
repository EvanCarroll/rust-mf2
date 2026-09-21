//! `p08-allocs` — allocation counts per operation (B10: 0 allocs for a simple
//! message, ≤ 1 for a 1-argument pattern), plus the fixture export for the
//! browser harness (`--emit DIR`).
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use p03_rt::{Arg, BidiStrategy, Catalog, Formatter, MsgId, NoErrors};
use p08_bench::fixtures;

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator, only counting.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size(), Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        REALLOCS.fetch_add(1, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count<R>(f: impl FnOnce() -> R) -> (R, usize, usize, usize) {
    let (a, r, b) = (ALLOCS.load(Relaxed), REALLOCS.load(Relaxed), BYTES.load(Relaxed));
    let out = f();
    (out, ALLOCS.load(Relaxed) - a, REALLOCS.load(Relaxed) - r, BYTES.load(Relaxed) - b)
}

fn main() {
    let emit = std::env::args().skip_while(|a| a != "--emit").nth(1);
    let fx = fixtures();
    println!("| locale | bytes | Catalog::new allocs / reallocs / bytes allocated | simple: allocs over all simple ids | 1-arg pattern (reused String) allocs / op | 1-arg pattern (new String::new()) allocs+reallocs / op | 1-arg pattern (String::with_capacity(128)) allocs / op | select (reused) allocs / op |");
    println!("|---|---|---|---|---|---|---|---|");
    for f in &fx {
        let bytes = f.bytes.clone();
        let (cat, a, r, b) = count(|| Catalog::new(bytes, f.hash));
        let cat = cat.unwrap_or_else(|_| panic!("load"));
        let fmt = Formatter::new(&cat, BidiStrategy::Default);
        let (_, sa, _, _) = count(|| f.simple.iter().map(|&i| fmt.simple(MsgId(i)).map_or(0, str::len)).sum::<usize>());
        let args = [Arg::Str("Ada Lovelace")];
        let mut out = String::with_capacity(1024);
        let (_, pa, pr, _) = count(|| {
            for &i in &f.pattern1 {
                out.clear();
                fmt.write(MsgId(i), &args, &mut out, &mut NoErrors);
            }
        });
        let n = f.pattern1.len() as f64;
        let (_, na, nr, _) = count(|| {
            for &i in &f.pattern1 {
                let mut s = String::new();
                fmt.write(MsgId(i), &args, &mut s, &mut NoErrors);
                std::hint::black_box(s);
            }
        });
        let (_, ca, cr, _) = count(|| {
            for &i in &f.pattern1 {
                let mut s = String::with_capacity(128);
                fmt.write(MsgId(i), &args, &mut s, &mut NoErrors);
                std::hint::black_box(s);
            }
        });
        let (_, xa, xr, _) = count(|| {
            for &i in &f.select {
                for c in 0..30 {
                    out.clear();
                    fmt.write(MsgId(i), &[Arg::Int(c)], &mut out, &mut NoErrors);
                }
            }
        });
        println!(
            "| {} | {} | {a} / {r} / {b} | {sa} over {} | {:.2} | {:.2} | {:.2} | {:.2} |",
            f.tag,
            f.bytes.len(),
            f.simple.len(),
            (pa + pr) as f64 / n,
            (na + nr) as f64 / n,
            (ca + cr) as f64 / n,
            (xa + xr) as f64 / (f.select.len() * 30) as f64
        );
    }
    if let Some(dir) = emit {
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut json = String::from("{\n");
        for (k, f) in fx.iter().enumerate() {
            std::fs::write(format!("{dir}/{}.mf2b", f.tag), &f.bytes).expect("write");
            let list = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
            json.push_str(&format!(
                "  \"{}\": {{\"hash_lo\": {}, \"hash_hi\": {}, \"bytes\": {}, \"pool_start\": {}, \"simple\": [{}], \"pattern1\": [{}], \"select\": [{}]}}{}\n",
                f.tag,
                f.hash & 0xffff_ffff,
                f.hash >> 32,
                f.bytes.len(),
                f.pool_start,
                list(&f.simple),
                list(&f.pattern1),
                list(&f.select),
                if k + 1 < fx.len() { "," } else { "" }
            ));
        }
        json.push('}');
        std::fs::write(format!("{dir}/fixtures.json"), json).expect("write");
        eprintln!("wrote catalogs + fixtures.json to {dir}");
    }
}
