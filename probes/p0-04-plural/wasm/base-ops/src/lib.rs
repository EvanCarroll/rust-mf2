//! Size baseline for `wasm-eval-ops`: the same scaffolding, no evaluator.
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn select_str(rptr: *const u8, rlen: usize, nptr: *const u8, nlen: usize) -> u32 {
    let entry = unsafe { core::slice::from_raw_parts(rptr, rlen) };
    let num = unsafe { core::slice::from_raw_parts(nptr, nlen) };
    u32::from(entry.first().copied().unwrap_or(0)) ^ u32::from(num.first().copied().unwrap_or(0))
}
