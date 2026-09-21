//! The plural evaluator alone: encoded entry + operands -> category code.
#![no_std]
#![allow(clippy::missing_safety_doc, clippy::too_many_arguments)]

unsafe extern "C" {
    /// Becomes a wasm import only if some panic path survives optimisation:
    /// its absence from the module proves the code is panic-free (B12).
    fn p04_panic_reachable() -> !;
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    unsafe { p04_panic_reachable() }
}

use plural_eval::{Operands, select as eval};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn select(ptr: *const u8, len: usize, i: u64, f: u64, t: u64, v: u32, w: u32, e: u32) -> u32 {
    let entry = unsafe { core::slice::from_raw_parts(ptr, len) };
    eval(entry, &Operands { i, f, t, v, w, e }) as u32
}
