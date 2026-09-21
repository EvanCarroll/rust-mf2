//! The runtime floor inside a std cdylib (std allocator + panic runtime, as in an app).
#![allow(clippy::missing_safety_doc)]


use std::vec::Vec;


use p03_rt::{Arg, BidiStrategy, Catalog, Formatter, MsgId, NoErrors, Part, PartSink, Sink};

/// Fixed output buffer (truncates; no allocation, no panic path in the harness).
struct Out {
    buf: [u8; 4096],
    len: usize,
}

impl Out {
    fn put(&mut self, s: &[u8]) {
        if let Some(d) = self.buf.get_mut(self.len..).and_then(|r| r.get_mut(..s.len())) {
            for (d, &b) in d.iter_mut().zip(s) {
                *d = b;
            }
            self.len = self.len.wrapping_add(s.len());
        }
    }
}

impl Sink for Out {
    fn push_str(&mut self, s: &str) {
        self.put(s.as_bytes());
    }
}

impl PartSink for Out {
    fn part(&mut self, p: Part<'_>) {
        let s = match p {
            Part::Text(t) => t,
            Part::BidiIsolation(i) => i.as_str(),
            Part::Expression { value, .. } => value,
            Part::Fallback(src) => {
                src.write(self);
                return;
            }
            Part::Markup { name, options, .. } => {
                for (n, _) in options {
                    self.put(n.as_bytes());
                }
                name
            }
        };
        self.put(s.as_bytes());
    }
}

use std::alloc::alloc as ALLOC_FN;
static mut OUT: Out = Out { buf: [0; 4096], len: 0 };
static mut CAT: Option<Catalog> = None;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alloc(len: usize) -> *mut u8 {
    // Straight to the allocator: no capacity check, no panic path in the harness.
    unsafe { ALLOC_FN(core::alloc::Layout::from_size_align_unchecked(len, 1)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn load(ptr: *mut u8, len: usize, hash_lo: u32, hash_hi: u32) -> u32 {
    let v = unsafe { Vec::from_raw_parts(ptr, len, len) };
    let hash = u64::from(hash_lo) | (u64::from(hash_hi) << 32);
    match Catalog::new(v, hash) {
        Ok(c) => {
            let n = c.len();
            unsafe { CAT = Some(c) };
            n
        }
        Err(_) => 0,
    }
}

fn cat() -> Option<&'static Catalog> {
    unsafe { (*core::ptr::addr_of!(CAT)).as_ref() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple(id: u32) -> u32 {
    cat().and_then(|c| Formatter::new(c, BidiStrategy::Default).simple(MsgId(id))).map_or(0, |s| s.len() as u32)
}

fn args(a0: i64, a1: u32) -> [Arg<'static>; 2] {
    [Arg::Int(a0), if a1 == 0 { Arg::Unset } else { Arg::Str("x") }]
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn format(id: u32, a0: i64, a1: u32) -> u32 {
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    out.len = 0;
    if let Some(c) = cat() {
        Formatter::new(c, BidiStrategy::Default).write(MsgId(id), &args(a0, a1), out, &mut NoErrors);
    }
    out.len as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn parts(id: u32, a0: i64, a1: u32) -> u32 {
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    out.len = 0;
    if let Some(c) = cat() {
        Formatter::new(c, BidiStrategy::Default).parts(MsgId(id), &args(a0, a1), out, &mut NoErrors);
    }
    out.len as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn out_ptr() -> *const u8 {
    unsafe { (*core::ptr::addr_of!(OUT)).buf.as_ptr() }
}
