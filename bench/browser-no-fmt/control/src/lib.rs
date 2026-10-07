//! B12 positive control: `b12-base` plus one deliberate panic path (an
//! indexing bounds check) and one deliberate use of `core::fmt` (`write!` of
//! an integer). `check.sh` requires its panic import to be **present** and its
//! fmt symbols to be **found** — proof that each check can fail, i.e. that a
//! clean `b12-reader` means something.
#![no_std]
// The bounds check and the formatting are the point of this crate.
#![allow(clippy::indexing_slicing, clippy::cast_possible_truncation)]

use core::fmt::Write;

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};

/// A `core::fmt::Write` that forwards to the host.
struct HostWriter;

impl Write for HostWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sink_bytes(s.as_bytes());
        Ok(())
    }
}

/// `b12-base`'s `run`, plus `bytes[param(0)]` and `write!(…, "{}", param(1))`.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    let Some(bytes) = input(INPUT_CATALOG) else {
        return 1;
    };
    let Some(key) = input(INPUT_KEY) else {
        return 1;
    };
    sink(param(0) ^ param(1) ^ param(2) ^ param(3));
    sink(u64::from(bytes[param(0) as usize]));
    let _ = write!(HostWriter, "{}", param(1));
    sink_bytes(&key);
    sink_bytes(&bytes);
    0
}
