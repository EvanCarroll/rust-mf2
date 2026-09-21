//! B12 size base: the `b12-harness` scaffolding with the same host inputs and
//! sinks as `b12-reader`, without the reader. The reader's size is the delta
//! `b12-reader` − `b12-base` (`plans/06-size-and-perf.md` §3); the input
//! buffer is a real allocation that escapes to the host, so the allocator is
//! in the base, not in the delta.
#![no_std]

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};

/// Reads the inputs and hands them back.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    let Some(bytes) = input(INPUT_CATALOG) else {
        return 1;
    };
    let Some(key) = input(INPUT_KEY) else {
        return 1;
    };
    sink(param(0) ^ param(1) ^ param(2) ^ param(3));
    sink_bytes(&key);
    sink_bytes(&bytes);
    0
}
