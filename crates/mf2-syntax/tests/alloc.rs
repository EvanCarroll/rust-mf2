//! The D1 allocation targets, checked exactly with a counting allocator:
//! parsing a placeholder-free message to the model allocates nothing, and a
//! reused parser's CST path stops allocating once its arena has grown.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use mf2_syntax::{Parser, parse_model};

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

const PLACEHOLDER_FREE: &[&str] = &[
    "",
    "Send",
    "Can’t reach the server.",
    "  leading and trailing  ",
    "\u{200e}\u{3000}bidi and ideographic space",
    "a.b.c (dots after the start are text)",
];

#[test]
fn placeholder_free_messages_parse_to_the_model_without_allocating() {
    let mut parser = Parser::new();
    for src in PLACEHOLDER_FREE {
        let (parsed, n) = allocations(|| parse_model(src));
        assert_eq!(n, 0, "{src:?}");
        assert!(parsed.message.is_some() && parsed.diagnostics.is_empty());
        let (_, n) = allocations(|| parser.parse_model(src));
        assert_eq!(n, 0, "reused, {src:?}");
    }
}

#[test]
fn a_reused_parser_stops_allocating() {
    let mut parser = Parser::new();
    let msgs = [
        "Hello {$name}, you have {$count :number} new {#b}messages{/b}.",
        ".input {$n :integer} .match $n one {{one}} * {{other {$n}}}",
        "Send",
    ];
    // Warm up: the arena grows to the largest message.
    for m in msgs {
        let _ = parser.parse_cst(m);
    }
    for m in msgs {
        let (_, n) = allocations(|| parser.parse_cst(m).diagnostics().len());
        assert_eq!(n, 0, "{m:?}");
    }
}

#[test]
fn a_message_allocates_once_per_collection_it_owns() {
    // "Hi {$x}!" owns one vector (the three parts): the arena plus that one.
    let (_, fresh) = allocations(|| parse_model("Hi {$x}!"));
    let mut parser = Parser::new();
    let _ = parser.parse_model("Hi {$x}!");
    let (_, reused) = allocations(|| parser.parse_model("Hi {$x}!"));
    assert_eq!(reused, 1, "only the pattern's vector");
    assert!(fresh <= 2, "arena + pattern, got {fresh}");
}
