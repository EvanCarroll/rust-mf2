//! What Rust keeps of the numeric operand once `Intl` does the digits: the
//! `number-literal` syntax (Bad Operand vs Unsupported Operation), the exact
//! value as text for `Intl` (`Intl.NumberFormat` v3 formats a decimal
//! string exactly), `:offset`'s exact addition, `:percent`'s × 100, and the
//! exact-match comparison.
//!
//! The exact value is the runtime's public, bare [`Number`]
//! (`Number::parse`, `from_i64`, `from_f64`, `is_integer`, `write_plain`):
//! the unannotated-value path links that code in every client already
//! (`crates/mf2-runtime/src/unannotated.rs`), so it is not a cost of this
//! option — the digit plan, the rounding, the display and the plural
//! evaluator are what the probe leaves out.

use alloc::string::String;

use mf2_runtime::{Number, Sink};

/// Whether `b` matches `number-literal` (`number.md`, "Numeric Operands"):
/// `["-"] (0 / [1-9]*DIGIT) ["." 1*DIGIT] [e ["-"/"+"] 1*DIGIT]`. Syntax
/// only — the runtime's `split_literal` (`number/decimal.rs`) without its
/// exponent limit, which `Number::parse` applies.
pub(crate) fn is_number_literal(b: &[u8]) -> bool {
    let rest = match b.split_first() {
        Some((b'-', r)) => r,
        _ => b,
    };
    let mut i = 0;
    match rest.first() {
        Some(b'0') => i = 1,
        Some(b'1'..=b'9') => {
            while rest.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return false,
    }
    if rest.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while rest.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if matches!(rest.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(rest.get(i), Some(b'-' | b'+')) {
            i += 1;
        }
        let start = i;
        while rest.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == rest.len()
}

/// Text on the stack (the common case: a short number or key), spilling to
/// the heap past 120 bytes. Growth goes through the runtime's guarded
/// `Sink for String` (`try_reserve`; on failure the text is dropped).
pub(crate) struct Text {
    buf: [u8; 120],
    len: usize,
    heap: Option<String>,
}

impl Text {
    pub(crate) const fn new() -> Text {
        Text {
            buf: [0; 120],
            len: 0,
            heap: None,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match &self.heap {
            Some(s) => s,
            None => core::str::from_utf8(self.buf.get(..self.len).unwrap_or(&[])).unwrap_or(""),
        }
    }

    /// Appends the decimal digits of `n`.
    pub(crate) fn push_u32(&mut self, n: u32) {
        let mut d = [0u8; 10];
        let mut at = d.len();
        let mut n = n;
        loop {
            at -= 1;
            // `n % 10 < 10`.
            #[allow(clippy::cast_possible_truncation)]
            if let Some(slot) = d.get_mut(at) {
                *slot = b'0' + (n % 10) as u8;
            }
            n /= 10;
            if n == 0 || at == 0 {
                break;
            }
        }
        self.push_str(core::str::from_utf8(d.get(at..).unwrap_or(&[])).unwrap_or(""));
    }
}

impl Sink for Text {
    fn push_str(&mut self, s: &str) {
        if let Some(h) = &mut self.heap {
            Sink::push_str(h, s);
            return;
        }
        let end = self.len + s.len();
        if end <= self.buf.len() {
            // A zip, not `copy_from_slice`: no length-mismatch panic path (B12).
            for (d, &b) in self.buf.iter_mut().skip(self.len).zip(s.as_bytes()) {
                *d = b;
            }
            self.len = end;
            return;
        }
        let mut h = String::new();
        Sink::push_str(&mut h, self.as_str());
        Sink::push_str(&mut h, s);
        self.heap = Some(h);
    }
}

/// The exact value `n` as plain text (`-1234.5`, `-0`, no exponent).
pub(crate) fn plain(n: &Number) -> Text {
    let mut t = Text::new();
    n.write_plain(&mut t);
    t
}

/// `n × 10^scale` (`:percent`: 2), exact; `None` past the runtime's
/// operand limits.
pub(crate) fn scaled(n: &Number, scale: i8) -> Option<Number> {
    if scale == 0 {
        return Some(n.clone());
    }
    let mut t = plain(n);
    t.push_str("e");
    if scale < 0 {
        t.push_str("-");
    }
    t.push_u32(u32::from(scale.unsigned_abs()));
    Number::parse(t.as_str())
}

/// Compares written bytes with a key.
pub(crate) struct Cmp<'k> {
    key: &'k [u8],
    pos: usize,
    ok: bool,
}

impl<'k> Cmp<'k> {
    pub(crate) fn new(key: &'k str) -> Cmp<'k> {
        Cmp {
            key: key.as_bytes(),
            pos: 0,
            ok: true,
        }
    }

    pub(crate) fn done(&self) -> bool {
        self.ok && self.pos == self.key.len()
    }
}

impl Sink for Cmp<'_> {
    fn push_str(&mut self, s: &str) {
        let end = self.pos + s.len();
        self.ok &= self.key.get(self.pos..end) == Some(s.as_bytes());
        self.pos = end;
    }
}

/// `n + delta` (`:offset`: `delta` 0..=99, subtracted with `subtract`),
/// exact, in place on the plain text: the integer part moves by `delta`
/// with carry or borrow; only when the sum changes sign (|n| < delta) is
/// the fraction complemented. `None` past the runtime's operand limits
/// (Unsupported Operation, as the runtime reports).
///
/// Zero signs follow IEEE 754 addition: `-0 + -0 = -0`, any other exact
/// zero sum is `+0`, and `0 − 5 = −5`. (The runtime's `Decimal::add` at
/// 3fc4735 keeps `self.neg && other.neg` whenever either side is zero, so it
/// gives `{0 :offset subtract=5}` = `5` and `{-5 :offset add=0}` = `5`;
/// reported in the probe's RESULTS.)
#[allow(clippy::many_single_char_names)]
pub(crate) fn add_small(n: &Number, delta: u8, subtract: bool) -> Option<Number> {
    let p = plain(n);
    let s = p.as_str().as_bytes();
    let neg = s.first() == Some(&b'-');
    let mag = s.get(usize::from(neg)..)?;
    let point = mag.iter().position(|&c| c == b'.').unwrap_or(mag.len());
    let zero = mag.iter().all(|&c| c == b'0' || c == b'.');
    if delta == 0 {
        return if zero {
            Number::parse(if neg && subtract { "-0" } else { "0" })
        } else {
            Some(n.clone())
        };
    }
    // The magnitude, two spare leading digits for a carry.
    let mut buf = [b'0'; 128];
    if mag.len() + 2 > buf.len() {
        return None;
    }
    for (d, &c) in buf.iter_mut().skip(2).zip(mag) {
        *d = c;
    }
    let (head, digits) = buf.split_at_mut_checked(2)?;
    let digits = digits.get_mut(..mag.len())?;
    let int_value = mag
        .get(..point)?
        .iter()
        .try_fold(0u32, |a, &c| {
            (a < 1000).then(|| a * 10 + u32::from(c - b'0'))
        })
        .unwrap_or(u32::MAX);
    let (int, frac) = digits.split_at_mut_checked(point)?;
    let d = u32::from(delta);
    let out_neg;
    if zero || neg == subtract {
        // |n| + delta, carrying into the spare digits.
        out_neg = if zero { subtract } else { neg };
        let mut c = d;
        for slot in int.iter_mut().rev().chain(head.iter_mut().rev()) {
            let x = u32::from(*slot - b'0') + c % 10;
            c = c / 10 + u32::from(x >= 10);
            // `x % 10 < 10`.
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = b'0' + (x % 10) as u8;
            }
        }
    } else if int_value >= d {
        // |n| − delta (≥ 0), the sign of n.
        out_neg = neg;
        let mut c = d;
        for slot in int.iter_mut().rev() {
            let x = u32::from(*slot - b'0') + 10 - c % 10;
            c = c / 10 + u32::from(x < 10);
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = b'0' + (x % 10) as u8;
            }
        }
    } else {
        // delta − |n| > 0 with |n| < delta ≤ 99: (delta − int − 1) + (1 −
        // 0.frac), or delta − int when the fraction is zero; delta's sign.
        out_neg = subtract;
        let last = frac.iter().rposition(|&c| c != b'0' && c != b'.');
        let mut whole = d - int_value;
        if let Some(last) = last {
            whole -= 1;
            for (i, slot) in frac.iter_mut().enumerate().skip(1) {
                let x = *slot - b'0';
                *slot = b'0'
                    + match i.cmp(&last) {
                        core::cmp::Ordering::Less => 9 - x,
                        core::cmp::Ordering::Equal => 10 - x,
                        core::cmp::Ordering::Greater => 0,
                    };
            }
        }
        for slot in int.iter_mut().rev() {
            // `whole % 10 < 10`.
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = b'0' + (whole % 10) as u8;
            }
            whole /= 10;
        }
        if let Some(h) = head.last_mut() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *h = b'0' + (whole % 10) as u8;
            }
        }
    }
    // Leading zeros off (number-literal has none), then the sign.
    let all = buf.get(..mag.len() + 2)?;
    let first = all.iter().position(|&c| c != b'0').map_or(all.len(), |i| {
        if all.get(i) == Some(&b'.') { i - 1 } else { i }
    });
    let body = all.get(first.min(all.len().saturating_sub(1))..)?;
    let mut t = Text::new();
    if out_neg && body.iter().any(|&c| c != b'0' && c != b'.') {
        t.push_str("-");
    }
    t.push_str(core::str::from_utf8(body).ok()?);
    Number::parse(t.as_str())
}
