//! Fair std baseline: like `std-base`, but the copied Vec is kept alive, so the
//! allocator and std's alloc-error/panic runtime are linked here too. (In the
//! audit's base LLVM deletes the unused Vec, so deltas against it also count
//! the allocator.) Deltas of std-eval / icu-* are reported against both.
#![allow(clippy::missing_safety_doc)]

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cat(ptr: *const u8, len: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let v: Vec<u8> = s.to_vec();
    core::hint::black_box(&v);
    v.len() as u32 + n
}
