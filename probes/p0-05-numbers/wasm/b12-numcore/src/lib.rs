//! B12 harness (06 §3): `no_std`, a bump allocator, a panic handler that never
//! looks at its `PanicInfo`. Whatever `core::fmt` remains after LTO is
//! reachable from numcore / fixed_decimal.
#![no_std]
extern crate alloc;

#[cfg(target_arch = "wasm32")]
mod rt {
    use core::alloc::{GlobalAlloc, Layout};
    use core::cell::UnsafeCell;

    // If this import survives LTO + wasm-opt, some panic path is reachable.
    #[link(wasm_import_module = "probe")]
    unsafe extern "C" {
        fn panic_reached();
    }

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
        // SAFETY: an imported host function without arguments.
        unsafe { panic_reached() };
        core::arch::wasm32::unreachable()
    }

    struct Bump(UnsafeCell<usize>);
    // SAFETY: wasm32 here is single-threaded.
    unsafe impl Sync for Bump {}
    const HEAP: usize = 1 << 20;
    static mut ARENA: [u8; HEAP] = [0; HEAP];

    unsafe impl GlobalAlloc for Bump {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            // SAFETY: single-threaded; the arena is only handed out here.
            unsafe {
                let next = &mut *self.0.get();
                let start = (*next + l.align() - 1) & !(l.align() - 1);
                if start + l.size() > HEAP {
                    return core::ptr::null_mut();
                }
                *next = start + l.size();
                (&raw mut ARENA).cast::<u8>().add(start)
            }
        }
        unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
    }

    #[global_allocator]
    static A: Bump = Bump(UnsafeCell::new(0));
}

include!("../../opts.rs");

struct Hash(u32);
impl numcore::ByteSink for Hash {
    fn byte(&mut self, b: u8) {
        self.0 = self.0.wrapping_mul(31).wrapping_add(u32::from(b));
    }
}

#[cfg(feature = "loc")]
struct HashSink(u32);
#[cfg(feature = "loc")]
impl numloc::StrSink for HashSink {
    fn push_str(&mut self, s: &str) {
        for b in s.bytes() {
            self.0 = self.0.wrapping_mul(31).wrapping_add(u32::from(b));
        }
    }
    fn push_char(&mut self, c: char) {
        self.0 = self.0.wrapping_mul(31).wrapping_add(u32::from(c));
    }
}

/// fn-number path: `data` = LOCALE entries, then `opts`, `operand` (NUL-separated).
#[cfg(feature = "loc")]
#[unsafe(no_mangle)]
pub extern "C" fn run_loc(dptr: *const u8, dlen: usize, ptr: *const u8, len: usize, func_code: u8) -> u32 {
    // SAFETY: the host passes valid (ptr, len) pairs into linear memory.
    let data = unsafe { core::slice::from_raw_parts(dptr, dlen) };
    // SAFETY: as above.
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    let Ok(s) = core::str::from_utf8(bytes) else { return 0 };
    let cut = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    let (opts, operand) = (s.get(..cut).unwrap_or(""), s.get(cut + 1..).unwrap_or(""));
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let o = buf.get(..n).unwrap_or(&[]);
    let mut e = Errs(0);
    let op = numloc::LocOperand::Core(numcore::Operand::Str(operand));
    let v = match func_code {
        3 => numloc::resolve_percent(&op, o, &mut e),
        4 => numloc::currency::resolve(&op, o, data, &mut e),
        5 => numloc::unit::resolve(&op, o, &mut e),
        c => numcore::resolve(func(c), &numcore::Operand::Str(operand), o, &mut e)
            .map(|num| numloc::LocValue { num, kind: numloc::Kind::Number }),
    };
    let mut h = HashSink(0);
    let cat = |op: &numcore::PluralOperands, _: bool| if op.i == 1 { "one" } else { "other" };
    if let Some(v) = v {
        numloc::format(&v, data, func_code > 5, &cat, &mut h, &mut e);
    }
    h.0 ^ e.0
}

/// `opts`, `operand`, `key` separated by NUL bytes.
#[unsafe(no_mangle)]
pub extern "C" fn run(ptr: *const u8, len: usize, func_code: u8) -> u32 {
    // SAFETY: the host passes a valid (ptr, len) pair into linear memory.
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    let Ok(s) = core::str::from_utf8(bytes) else { return 0 };
    // NUL-separated, split without `str` patterns (no panicking slicing).
    let mut parts = [""; 3];
    let (mut k, mut start) = (0, 0);
    for (i, &c) in bytes.iter().enumerate() {
        if c == 0 {
            if let (Some(slot), Some(p)) = (parts.get_mut(k), s.get(start..i)) {
                *slot = p;
            }
            k += 1;
            start = i + 1;
        }
    }
    if let (Some(slot), Some(p)) = (parts.get_mut(k), s.get(start..)) {
        *slot = p;
    }
    let [opts, operand, key] = parts;
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let mut e = Errs(0);
    let mut h = Hash(0);
    let op = match operand.strip_prefix('#') {
        Some(d) => numcore::Operand::Int(d.bytes().fold(0i64, |a, c| a * 10 + i64::from(c & 15))),
        #[cfg(feature = "f64")]
        None if operand.starts_with('f') => numcore::Operand::Float(f64::from(operand.len() as u32) / 7.0),
        None => numcore::Operand::Str(operand),
    };
    #[cfg(feature = "only-number")]
    let fcode = { let _ = func_code; 0 };
    #[cfg(not(feature = "only-number"))]
    let fcode = func_code;
    if let Some(v) = numcore::resolve(func(fcode), &op, buf.get(..n).unwrap_or(&[]), &mut e) {
        #[cfg(not(feature = "no-format"))]
        {
            let f = numcore::format_digits(&v, &mut e);
            numcore::write_neutral(&f, &mut h);
            #[cfg(not(feature = "no-select"))]
            {
                h.0 ^= select_bits(&v, &f, key);
            }
        }
        #[cfg(feature = "no-format")]
        {
            h.0 ^= u32::from(v.selectable) ^ u32::from(v.value.absolute.digit_at(0)) ^ key.len() as u32;
        }
    }
    h.0 ^ e.0
}
