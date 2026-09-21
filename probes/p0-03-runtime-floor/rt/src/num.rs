//! Hand-written integer ↔ text (no `core::fmt`, B12).

/// Formats `n` in ASCII decimal into `buf` (20 bytes hold any `i64`).
pub(crate) fn fmt_i64(n: i64, buf: &mut [u8; 20]) -> &str {
    let mut u = n.unsigned_abs();
    let mut i = buf.len();
    loop {
        i = i.saturating_sub(1);
        if let Some(b) = buf.get_mut(i) {
            *b = b'0'.wrapping_add((u % 10) as u8);
        }
        u /= 10;
        if u == 0 || i == 0 {
            break;
        }
    }
    if n < 0 && i > 0 {
        i = i.saturating_sub(1);
        if let Some(b) = buf.get_mut(i) {
            *b = b'-';
        }
    }
    core::str::from_utf8(buf.get(i..).unwrap_or(&[])).unwrap_or("")
}

/// `-?[0-9]+` exactly (an integer number literal).
pub(crate) fn is_int_literal(s: &str) -> bool {
    let d = s.strip_prefix('-').unwrap_or(s);
    !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())
}

/// Parses `-?[0-9]+(\.[0-9]+)?`; with `truncate`, a fraction is dropped
/// (`:integer`), otherwise a fraction makes it not an integer (`None`).
pub(crate) fn parse_int(s: &str, truncate: bool) -> Option<i64> {
    let (neg, d) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (int, frac) = match d.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (d, None),
    };
    if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(f) = frac
        && (f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) || (!truncate && f.bytes().any(|b| b != b'0')))
    {
        return None;
    }
    let mut v: i64 = 0;
    for b in int.bytes() {
        v = v.checked_mul(10)?.checked_add(i64::from(b.wrapping_sub(b'0')))?;
    }
    Some(if neg { v.checked_neg()? } else { v })
}
