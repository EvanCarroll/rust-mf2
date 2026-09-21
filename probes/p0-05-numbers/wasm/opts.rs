// Probe harness only: `name=value` pairs separated by spaces, byte-wise (no
// `str` pattern machinery). The product gets options pre-split from the catalog.
fn parse_opts<'a>(s: &'a str, out: &mut [(&'a str, numcore::OptValue<'a>); 8]) -> usize {
    let b = s.as_bytes();
    let (mut n, mut start, mut eq) = (0, 0, None);
    for i in 0..=b.len() {
        let c = b.get(i).copied().unwrap_or(b' ');
        if c == b'=' && eq.is_none() {
            eq = Some(i);
        } else if c == b' ' {
            if let (Some(e), Some(slot)) = (eq, out.get_mut(n)) {
                if let (Some(k), Some(v)) = (s.get(start..e), s.get(e + 1..i)) {
                    *slot = (k, numcore::OptValue::Literal(v));
                    n += 1;
                }
            }
            start = i + 1;
            eq = None;
        }
    }
    n
}

fn func(code: u8) -> numcore::Func {
    match code {
        1 => numcore::Func::Integer,
        2 => numcore::Func::Offset,
        _ => numcore::Func::Number,
    }
}

struct Errs(u32);
impl numcore::ErrSink for Errs {
    fn push(&mut self, e: numcore::Error) {
        self.0 |= 1 << (e as u32);
    }
}

/// Keys the selection path so exact serialization + plural operands stay linked.
fn select_bits(v: &numcore::NumValue, f: &numcore::Formatted, key: &str) -> u32 {
    let cat = |op: &numcore::PluralOperands, ord: bool| -> &'static str {
        if op.i == 1 && op.v == 0 && !ord { "one" } else { "other" }
    };
    let m = numcore::matches(v, f, key, &cat);
    let op = numcore::plural_operands(f);
    (m == Ok(true)) as u32 | (op.v as u32) << 1 | (op.w as u32) << 6 | ((op.f ^ op.t ^ op.i) as u32) << 11
}
