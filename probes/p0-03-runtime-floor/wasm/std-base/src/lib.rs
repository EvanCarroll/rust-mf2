//! std size baseline for `wasm-std-floor`: same exports, std allocator and
//! panic runtime kept alive (the output Vec really allocates).
#![allow(clippy::missing_safety_doc)]


use std::vec::Vec;


use std::alloc::alloc as ALLOC_FN;
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
static mut OUT: Out = Out { buf: [0; 4096], len: 0 };
static mut CAT: Vec<u8> = Vec::new();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn alloc(len: usize) -> *mut u8 {
    // Straight to the allocator: no capacity check, no panic path in the harness.
    unsafe { ALLOC_FN(core::alloc::Layout::from_size_align_unchecked(len, 1)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn load(ptr: *mut u8, len: usize, hash_lo: u32, hash_hi: u32) -> u32 {
    let v = unsafe { Vec::from_raw_parts(ptr, len, len) };
    let r = v.first().copied().unwrap_or(0) as u32 ^ hash_lo ^ hash_hi;
    // Keep a real allocation (std allocator) alive, as the runtime's load does.
    let mut keep = std::hint::black_box(Vec::with_capacity(v.len() / 2));
    keep.extend_from_slice(v.get(v.len() / 2..).unwrap_or(&[]));
    unsafe { CAT = std::hint::black_box(keep) };
    r
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple(id: u32) -> u32 {
    let c = unsafe { &*core::ptr::addr_of!(CAT) };
    c.get(id as usize).copied().unwrap_or(0) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn format(id: u32, a0: i64, a1: u32) -> u32 {
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    out.len = 0;
    out.put(&a0.to_le_bytes());
    out.put(&[id as u8 ^ a1 as u8]);
    out.len as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn parts(id: u32, a0: i64, a1: u32) -> u32 {
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    out.len = 0;
    out.put(&[id as u8 ^ a0 as u8 ^ a1 as u8]);
    out.len as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn out_ptr() -> *const u8 {
    unsafe { (*core::ptr::addr_of!(OUT)).buf.as_ptr() }
}
