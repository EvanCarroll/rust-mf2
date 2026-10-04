//! Shared scaffolding of the B12 harnesses (`b12-base`, `b12-reader`,
//! `b12-control`), so that the base and the reader differ by the reader alone.
//!
//! * The `#[panic_handler]` calls the imported `b12::b12_panic_reachable`.
//!   After fat LTO and `wasm-opt -Oz` that import exists only if some panic
//!   path survives: its absence proves the module panic-free (B12).
//! * Every input comes from an import (the catalog bytes, a lookup key,
//!   numbers), so nothing is constant-folded; every result goes to an
//!   imported sink, so nothing is dead-code-eliminated.
//! * A bump `#[global_allocator]`; the input buffer is a real allocation whose
//!   pointer escapes to the host, so the allocator stays in the base (a size
//!   delta is only fair against a base that keeps an allocation alive).
//! * Panic-free itself: no indexing, no `unwrap`, no infallible growth, so a
//!   surviving panic import can only come from the code under test.
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Host input: the `.mf2b` catalog.
pub const INPUT_CATALOG: u32 = 0;
/// Host input: a message id (name) for `Catalog::lookup`.
pub const INPUT_KEY: u32 = 1;

#[link(wasm_import_module = "b12")]
unsafe extern "C" {
    /// Called only by the panic handler.
    fn b12_panic_reachable() -> !;
    /// The length of host input `which`.
    fn b12_input_len(which: u32) -> usize;
    /// Copies host input `which` (exactly `b12_input_len(which)` bytes) to `dst`.
    fn b12_input_read(which: u32, dst: *mut u8);
    /// A number chosen by the host.
    fn b12_param(which: u32) -> u64;
    /// Receives a number.
    fn b12_sink(v: u64);
    /// Receives bytes.
    fn b12_sink_bytes(ptr: *const u8, len: usize);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // SAFETY: an import with no preconditions.
    unsafe { b12_panic_reachable() }
}

/// A bump allocator over `memory.grow`; `dealloc` is a no-op.
struct Bump;

static NEXT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: returns a fresh, suitably aligned block of `l.size()` bytes, or null.
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let mut p = NEXT.load(Ordering::Relaxed);
        if p == 0 {
            p = core::arch::wasm32::memory_size(0) << 16;
        }
        let Some(a) = p.checked_add(l.align() - 1).map(|x| x & !(l.align() - 1)) else {
            return core::ptr::null_mut();
        };
        let Some(end) = a.checked_add(l.size()) else {
            return core::ptr::null_mut();
        };
        let have = core::arch::wasm32::memory_size(0) << 16;
        if end > have
            && core::arch::wasm32::memory_grow(0, (end - have).div_ceil(0x1_0000)) == usize::MAX
        {
            return core::ptr::null_mut();
        }
        NEXT.store(end, Ordering::Relaxed);
        a as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Bump = Bump;

/// Host input `which`, in a fresh allocation; `None` if allocation fails.
pub fn input(which: u32) -> Option<Vec<u8>> {
    // SAFETY: an import with no preconditions.
    let len = unsafe { b12_input_len(which) };
    if len == 0 {
        return Some(Vec::new());
    }
    let layout = Layout::from_size_align(len, 1).ok()?;
    // SAFETY: `layout` has a non-zero size.
    let p = unsafe { alloc::alloc::alloc(layout) };
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is valid for `len` bytes, which the host initialises; the
    // block was allocated by the global allocator with capacity `len`.
    #[allow(clippy::same_length_and_capacity)] // exactly `len` was allocated
    unsafe {
        b12_input_read(which, p);
        Some(Vec::from_raw_parts(p, len, len))
    }
}

/// A number chosen by the host.
pub fn param(which: u32) -> u64 {
    // SAFETY: an import with no preconditions.
    unsafe { b12_param(which) }
}

/// Hands a number to the host.
pub fn sink(v: u64) {
    // SAFETY: an import with no preconditions.
    unsafe { b12_sink(v) }
}

/// Hands bytes to the host (the pointer escapes, so the allocation behind
/// them stays alive).
pub fn sink_bytes(b: &[u8]) {
    // SAFETY: `b` is valid for `b.len()` bytes for the duration of the call.
    unsafe { b12_sink_bytes(b.as_ptr(), b.len()) }
}
