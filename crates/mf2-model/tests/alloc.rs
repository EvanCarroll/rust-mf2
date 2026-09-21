//! The D1 target at the model level: a placeholder-free message is built
//! without a single allocation (one pattern part is stored inline).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use mf2_model::{Cow, Diagnostics, Message, Parsed, Pattern, PatternMessage};

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

// A test-only counting allocator; every method forwards unchanged to `System`.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: forwarded unchanged; the caller upholds the contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged; `ptr` came from `System`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: forwarded unchanged; the caller upholds the contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let before = ALLOCS.with(Cell::get);
    let r = f();
    (r, ALLOCS.with(Cell::get) - before)
}

#[test]
fn placeholder_free_message_needs_no_allocation() {
    let source = String::from("Send the message");
    let (parsed, n) = allocations(|| Parsed {
        message: Some(Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern: Pattern::from_text(Cow::Borrowed(&source)),
        })),
        diagnostics: Diagnostics::new(),
    });
    assert_eq!(n, 0);
    let Some(Message::Pattern(m)) = &parsed.message else {
        panic!("pattern message")
    };
    assert_eq!(m.pattern.as_simple_text(), Some("Send the message"));
}
