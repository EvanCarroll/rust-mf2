//! Hand-rolled UTS #35 plural rule evaluator over a compact byte encoding.
//! Layout: [n_categories u8] then per category: [cat u8][n_and_groups u8] then per group:
//! [n_relations u8] then per relation: [operand u8][negate u8][modulus u32le][n_ranges u8]([lo u32le][hi u32le])*
//! Operands are computed from a decimal string (sign-stripped), supporting n,i,v,w,f,t (c/e = 0).
#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

struct Ops { i: u64, v: u32, w: u32, f: u64, t: u64 }

fn operands(s: &[u8]) -> Option<Ops> {
    let mut i = 0u64; let mut f = 0u64; let mut v = 0u32; let mut seen_dot = false;
    for &b in s {
        match b {
            b'0'..=b'9' => {
                let d = (b - b'0') as u64;
                if seen_dot { f = f.checked_mul(10)?.checked_add(d)?; v += 1; } else { i = i.checked_mul(10)?.checked_add(d)?; }
            }
            b'.' if !seen_dot => seen_dot = true,
            _ => return None,
        }
    }
    let (mut t, mut w) = (f, v);
    while w > 0 && t % 10 == 0 { t /= 10; w -= 1; }
    Some(Ops { i, v, w, f, t })
}

struct Rd<'a>(&'a [u8]);
impl Rd<'_> {
    fn u8(&mut self) -> Option<u8> { let (&b, r) = self.0.split_first()?; self.0 = r; Some(b) }
    fn u32(&mut self) -> Option<u32> { let (h, r) = self.0.split_first_chunk::<4>()?; self.0 = r; Some(u32::from_le_bytes(*h)) }
}

fn eval(rules: &[u8], o: &Ops) -> Option<u8> {
    let mut r = Rd(rules);
    for _ in 0..r.u8()? {
        let cat = r.u8()?;
        let mut any = false;
        for _ in 0..r.u8()? {
            let mut all = true;
            for _ in 0..r.u8()? {
                let operand = r.u8()?; let negate = r.u8()? != 0; let modulus = r.u32()? as u64;
                // `n` only matches integer ranges when there is no visible fraction (t == 0).
                let (val, is_int) = match operand { 0 => (o.i, o.t == 0), 1 => (o.i, true), 2 => (o.v as u64, true), 3 => (o.w as u64, true), 4 => (o.f, true), 5 => (o.t, true), _ => (0, true) };
                let val = if modulus != 0 { val % modulus } else { val };
                let mut hit = false;
                for _ in 0..r.u8()? { let lo = r.u32()? as u64; let hi = r.u32()? as u64; hit |= is_int && val >= lo && val <= hi; }
                all &= hit != negate;
            }
            any |= all;
        }
        if any { return Some(cat); }
    }
    Some(5)
}

#[unsafe(no_mangle)]
pub extern "C" fn cat(rptr: *const u8, rlen: usize, nptr: *const u8, nlen: usize) -> u32 {
    let rules = unsafe { core::slice::from_raw_parts(rptr, rlen) };
    let num = unsafe { core::slice::from_raw_parts(nptr, nlen) };
    match operands(num).and_then(|o| eval(rules, &o)) { Some(c) => c as u32, None => 99 }
}
