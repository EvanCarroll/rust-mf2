//! P0.8 browser harness: the P0.3 runtime (release profile of 06 §3, then
//! wasm-opt -Oz) driven from JS. Each `bench_*` runs `n` operations so the JS
//! timer (5 µs resolution when cross-origin isolated) brackets many of them.
#![forbid(unsafe_code)]

use std::cell::RefCell;

use p03_rt::{Arg, BidiStrategy, Catalog, Formatter, MsgId, NoErrors};
use wasm_bindgen::prelude::*;

thread_local! {
    static STAGED: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static CAT: RefCell<Option<Catalog>> = const { RefCell::new(None) };
    static IDS: RefCell<[Vec<u32>; 3]> = const { RefCell::new([Vec::new(), Vec::new(), Vec::new()]) };
    static OUT: RefCell<String> = RefCell::new(String::with_capacity(1024));
}

fn hash(lo: u32, hi: u32) -> u64 {
    u64::from(lo) | (u64::from(hi) << 32)
}

/// The real install path: bytes from JS (wasm-bindgen copies the Uint8Array
/// into wasm memory) → `Catalog::new`. Returns the message count, 0 on error.
#[wasm_bindgen]
pub fn load(bytes: Vec<u8>, lo: u32, hi: u32) -> u32 {
    match Catalog::new(bytes, hash(lo, hi)) {
        Ok(c) => {
            let n = c.len();
            CAT.with(|x| *x.borrow_mut() = Some(c));
            n
        }
        Err(_) => 0,
    }
}

/// Keeps a copy in wasm memory for the repeated-load benchmark.
#[wasm_bindgen]
pub fn stage(bytes: Vec<u8>) {
    STAGED.with(|s| *s.borrow_mut() = bytes);
}

#[wasm_bindgen]
pub fn set_ids(class: u32, ids: Vec<u32>) {
    IDS.with(|x| {
        if let Some(v) = x.borrow_mut().get_mut(class as usize) {
            *v = ids;
        }
    });
}

/// Loads the staged buffer itself (moved, not cloned): `Catalog::new` alone.
#[wasm_bindgen]
pub fn load_staged(lo: u32, hi: u32) -> u32 {
    let bytes = STAGED.with(|s| std::mem::take(&mut *s.borrow_mut()));
    match Catalog::new(bytes, hash(lo, hi)) {
        Ok(c) => {
            let n = c.len();
            CAT.with(|x| *x.borrow_mut() = Some(c));
            n
        }
        Err(_) => 0,
    }
}

/// `n` × (clone of the staged bytes + `Catalog::new`).
#[wasm_bindgen]
pub fn bench_load(n: u32, lo: u32, hi: u32) -> u32 {
    STAGED.with(|s| {
        let s = s.borrow();
        let mut ok = 0;
        for _ in 0..n {
            ok += u32::from(Catalog::new(s.clone(), hash(lo, hi)).is_ok());
        }
        ok
    })
}

/// `n` × clone of the staged bytes (subtracted from `bench_load`).
#[wasm_bindgen]
pub fn bench_clone(n: u32) -> u32 {
    STAGED.with(|s| {
        let s = s.borrow();
        let mut t = 0u32;
        for _ in 0..n {
            t = t.wrapping_add(std::hint::black_box(s.clone()).len() as u32);
        }
        t
    })
}

/// `n` × `str::from_utf8` over the staged pool (`pool_start..`).
#[wasm_bindgen]
pub fn bench_utf8(n: u32, pool_start: u32) -> u32 {
    STAGED.with(|s| {
        let s = s.borrow();
        let pool = s.get(pool_start as usize..).unwrap_or(&[]);
        let mut ok = 0;
        for _ in 0..n {
            ok += u32::from(std::str::from_utf8(std::hint::black_box(pool)).is_ok());
        }
        ok
    })
}

fn with_fmt<R>(f: impl FnOnce(&Formatter<'_>, &[Vec<u32>; 3], &mut String) -> R) -> Option<R> {
    CAT.with(|c| {
        let c = c.borrow();
        let cat = c.as_ref()?;
        let fmt = Formatter::new(cat, BidiStrategy::Default);
        Some(IDS.with(|ids| OUT.with(|o| f(&fmt, &ids.borrow(), &mut o.borrow_mut()))))
    })
}

/// `n` simple lookups (fast path), cycling over the simple ids.
#[wasm_bindgen]
pub fn bench_simple(n: u32) -> u32 {
    with_fmt(|fmt, ids, _| {
        let v = &ids[0];
        let mut t = 0u32;
        for k in 0..n as usize {
            let id = v.get(k % v.len().max(1)).copied().unwrap_or(0);
            t = t.wrapping_add(fmt.simple(MsgId(id)).map_or(0, |s| s.len() as u32));
        }
        t
    })
    .unwrap_or(0)
}

/// `n` 1-argument pattern formats into a reused String.
#[wasm_bindgen]
pub fn bench_pattern(n: u32) -> u32 {
    with_fmt(|fmt, ids, out| {
        let v = &ids[1];
        let args = [Arg::Str("Ada Lovelace")];
        let mut t = 0u32;
        for k in 0..n as usize {
            out.clear();
            fmt.write(MsgId(v.get(k % v.len().max(1)).copied().unwrap_or(0)), &args, out, &mut NoErrors);
            t = t.wrapping_add(out.len() as u32);
        }
        t
    })
    .unwrap_or(0)
}

/// `n` select formats (plural on `$count`) into a reused String.
#[wasm_bindgen]
pub fn bench_select(n: u32) -> u32 {
    with_fmt(|fmt, ids, out| {
        let v = &ids[2];
        let mut t = 0u32;
        for k in 0..n as usize {
            out.clear();
            let args = [Arg::Int((k / v.len().max(1) % 30) as i64)];
            fmt.write(MsgId(v.get(k % v.len().max(1)).copied().unwrap_or(0)), &args, out, &mut NoErrors);
            t = t.wrapping_add(out.len() as u32);
        }
        t
    })
    .unwrap_or(0)
}

/// Formats one message (spot check from JS).
#[wasm_bindgen]
pub fn format_one(id: u32, s: &str, n: i32) -> String {
    with_fmt(|fmt, _, out| {
        out.clear();
        let args = if s.is_empty() { [Arg::Int(i64::from(n))] } else { [Arg::Str(s)] };
        fmt.write(MsgId(id), &args, out, &mut NoErrors);
        out.clone()
    })
    .unwrap_or_default()
}
