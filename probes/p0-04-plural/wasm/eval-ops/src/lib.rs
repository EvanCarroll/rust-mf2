//! Evaluator plus operand extraction from a decimal string (test-side path).
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

use plural_eval::{Operands, select};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn select_str(rptr: *const u8, rlen: usize, nptr: *const u8, nlen: usize) -> u32 {
    let entry = unsafe { core::slice::from_raw_parts(rptr, rlen) };
    let num = unsafe { core::slice::from_raw_parts(nptr, nlen) };
    Operands::parse(num).map_or(99, |o| select(entry, &o) as u32)
}
