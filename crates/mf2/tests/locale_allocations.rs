//! The one matcher reads a tag where it lies:
//! choosing a language allocates nothing. 1.x's
//! native matcher copied the tag into a `String` on every `set_locale` and
//! `with_locale`. A binary of its own: its allocator counts what this
//! thread allocates, and the store is process-wide.

#![allow(unsafe_code, reason = "a counting GlobalAlloc that forwards to System")]

#[path = "support/corpus.rs"]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use mf2::native;

struct Counting;

std::thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
}

// SAFETY: each method forwards to `System` unchanged, with the caller's
// arguments, after counting; `GlobalAlloc`'s contract passes through.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// What `body` returns, and how many allocations this thread made in it.
fn allocations<R>(body: impl FnOnce() -> R) -> (R, usize) {
    let before = ALLOCATIONS.with(Cell::get);
    let out = body();
    (out, ALLOCATIONS.with(Cell::get) - before)
}

#[test]
fn choosing_a_language_allocates_nothing() {
    let (corpus, _) = support::corpus(true);
    native::install(corpus);
    for tag in ["fr", "fr_CA.UTF-8", "FR-ca", "en-GB", "en_US.UTF-8@euro"] {
        let (chosen, n) = allocations(|| native::set_locale(tag));
        assert!(chosen.is_ok(), "{tag}");
        assert_eq!(n, 0, "set_locale({tag:?})");
        let (chosen, n) = allocations(|| native::with_locale(corpus, tag, native::locale));
        assert!(chosen.is_ok(), "{tag}");
        assert_eq!(n, 0, "with_locale({tag:?})");
    }
    // A language nothing serves is an error naming it: that one allocates.
    let (refused, n) = allocations(|| native::set_locale("de"));
    assert!(refused.is_err());
    assert!(n > 0);
}
