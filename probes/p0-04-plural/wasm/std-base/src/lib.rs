//! Audit-style std baseline (probes/audit/plural-size/base, unchanged logic).
#![allow(clippy::missing_safety_doc)]

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cat(ptr: *const u8, len: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let v: Vec<u8> = s.to_vec();
    v.len() as u32 + n
}
