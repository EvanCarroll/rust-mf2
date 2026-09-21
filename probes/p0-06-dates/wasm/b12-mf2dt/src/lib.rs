//! B12 harness (06 §3): `no_std`, no allocator, a panic handler that never
//! looks at its `PanicInfo`. Whatever `core::fmt` remains after LTO is
//! reachable from the code under test.
#![no_std]

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable()
}

struct Errs(u32);
impl mf2dt::ErrSink for Errs {
    fn push(&mut self, e: mf2dt::Error) {
        self.0 |= 1 << (e as u32);
    }
}

/// `opts` and `operand` separated by a NUL byte.
#[unsafe(no_mangle)]
pub extern "C" fn run(ptr: *const u8, len: usize, func: u8) -> u32 {
    // SAFETY: the host passes a valid (ptr, len) pair into linear memory.
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    let Ok(s) = core::str::from_utf8(bytes) else { return 0 };
    let (opts, operand) = s.split_once('\0').unwrap_or((s, ""));
    let mut e = Errs(0);
    let mut acc = 0u32;
    if let Some((p, z)) = mf2dt::harness::plan(func, opts, operand, &mut e) {
        let c = match z {
            mf2dt::Zoned::Fixed { civil, .. } | mf2dt::Zoned::Named { civil, .. } => civil,
        };
        let mut b = [0u8; 19];
        mf2dt::write_neutral(&c, &mut b);
        acc = b.iter().fold(0u32, |a, &x| a.wrapping_mul(31).wrapping_add(u32::from(x)));
        acc ^= (p.length as u32) << 8 | p.date.map_or(0, |d| d as u32 + 1) << 12;
        acc ^= p.time.map_or(0, |t| t as u32 + 1) << 16 | p.zone.map_or(0, |t| t as u32 + 1) << 20;
        acc ^= u32::from(c.iso_weekday()) << 24;
    }
    acc ^ e.0
}
