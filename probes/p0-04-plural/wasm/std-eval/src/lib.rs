//! Our evaluator in the audit's std harness: rules blob (copied, like the ICU
//! blob) + integer -> category, for a like-for-like comparison with icu_plurals.
#![allow(clippy::missing_safety_doc)]

use plural_eval::{Operands, select};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cat(ptr: *const u8, len: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let v: Vec<u8> = s.to_vec();
    select(&v, &Operands { i: u64::from(n), ..Operands::default() }) as u32
}
