//! Size baseline for `wasm-eval`: the same scaffolding, no evaluator.
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
pub unsafe extern "C" fn select(ptr: *const u8, len: usize, i: u64, f: u64, t: u64, v: u32, w: u32, e: u32) -> u32 {
    let entry = unsafe { core::slice::from_raw_parts(ptr, len) };
    u32::from(entry.first().copied().unwrap_or(0)) ^ ((i ^ f ^ t) as u32) ^ v ^ w ^ e
}
