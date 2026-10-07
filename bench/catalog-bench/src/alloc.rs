//! The counting global allocator (the parser gate's, `bench/parser-vs-ox`).
//!
//! Every allocation made by the **calling thread** is counted: the number of
//! calls to `alloc`, `alloc_zeroed` and `realloc`, and the bytes they request
//! (a `realloc` counts as one allocation of its new size). Deallocations are
//! not counted. Counters are thread-local, so parallel unit tests cannot
//! perturb each other; the benchmark itself is single-threaded.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    // `const`-initialised and without `Drop`: reading them never allocates and
    // never fails, which is what an allocator needs.
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

#[inline]
fn record(size: usize) {
    let _ = ALLOCS.try_with(|c| c.set(c.get().wrapping_add(1)));
    let _ = BYTES.try_with(|c| c.set(c.get().wrapping_add(size as u64)));
}

/// The system allocator, counting as described in the module documentation.
pub struct Counting;

// The one `unsafe` item of the crate (the workspace denies `unsafe_code`): a
// `GlobalAlloc` impl is `unsafe` by definition. Every method forwards its
// arguments unchanged to `System`, so the caller's obligations under the
// `GlobalAlloc` contract are passed through untouched.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: forwarded unchanged; the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: forwarded unchanged; the caller upholds `GlobalAlloc::alloc_zeroed`'s contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged; `ptr` was returned by this allocator (i.e. by `System`).
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: forwarded unchanged; the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Allocation counters of the calling thread at one instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Counts {
    /// Allocation calls (`alloc`, `alloc_zeroed`, `realloc`).
    pub allocs: u64,
    /// Bytes requested by those calls.
    pub bytes: u64,
}

impl Counts {
    /// The allocations made between `earlier` and `self`.
    #[must_use]
    pub fn since(self, earlier: Self) -> Self {
        Self {
            allocs: self.allocs.wrapping_sub(earlier.allocs),
            bytes: self.bytes.wrapping_sub(earlier.bytes),
        }
    }
}

/// The calling thread's counters now. Does not allocate.
pub fn snapshot() -> Counts {
    Counts {
        allocs: ALLOCS.try_with(Cell::get).unwrap_or(0),
        bytes: BYTES.try_with(Cell::get).unwrap_or(0),
    }
}

/// Runs `f` and returns its result with the allocations it made on this thread.
pub fn count<R>(f: impl FnOnce() -> R) -> (R, Counts) {
    let before = snapshot();
    let out = f();
    (out, snapshot().since(before))
}

#[cfg(test)]
mod tests {
    use super::count;

    #[test]
    fn counts_one_allocation_and_its_bytes() {
        let (v, used) = count(|| Vec::<u64>::with_capacity(10));
        assert_eq!(used.allocs, 1);
        assert_eq!(used.bytes, 80);
        drop(v);
    }

    #[test]
    fn no_allocation_counts_zero() {
        let (sum, used) = count(|| (1..=10u64).sum::<u64>());
        assert_eq!(sum, 55);
        assert_eq!(used.allocs, 0);
    }
}
