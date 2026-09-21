#[unsafe(no_mangle)]
pub extern "C" fn cat(ptr: *const u8, len: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let v: Vec<u8> = s.to_vec();
    v.len() as u32 + n
}
