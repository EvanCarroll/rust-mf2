//! The plural-rule evaluator (decision D3; P0.4): one encoded
//! `plural.cardinal` / `plural.ordinal` LOCALE entry
//! (`plans/02-catalog-format.md` §4.1) applied to the UTS #35 operands of a
//! *formatted* number. No allocation, no panic: malformed data stops the
//! evaluation with `other`, the spec's catch-all.

/// Operand codes (relation header bits 0–2); `c` and `e` share code 6.
mod op {
    pub(super) const N: u8 = 0;
    pub(super) const I: u8 = 1;
    pub(super) const V: u8 = 2;
    pub(super) const W: u8 = 3;
    pub(super) const F: u8 = 4;
    pub(super) const T: u8 = 5;
    pub(super) const E: u8 = 6;
}

/// Modulus field value: an explicit LEB128 modulus follows.
const MOD_EXPLICIT: u8 = 7;
/// Relation header bit 6: `!=`.
const REL_NEGATED: u8 = 0x40;
/// Relation header bit 7: last relation of its AND group.
const REL_LAST: u8 = 0x80;
/// Item bit 0: last item of the list.
const ITEM_LAST: u64 = 1;
/// Item bit 1: a range; `hi − lo` follows.
const ITEM_RANGE: u64 = 2;
/// Rule header bits 0–4: the number of OR groups.
const RULE_GROUPS_MASK: u8 = 0x1f;

/// 10^18: operand values at or above it are kept as `10^18 + (value mod
/// 10^18)` — exact for every CLDR modulus and literal (§4.1).
pub(crate) const BIG: u64 = 1_000_000_000_000_000_000;

/// A CLDR plural category.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Category {
    /// `zero`
    Zero = 0,
    /// `one`
    One = 1,
    /// `two`
    Two = 2,
    /// `few`
    Few = 3,
    /// `many`
    Many = 4,
    /// `other`
    Other = 5,
}

impl Category {
    /// The category of a rule-header code; codes ≥ 5 are `other`.
    pub const fn from_code(code: u8) -> Category {
        match code {
            0 => Category::Zero,
            1 => Category::One,
            2 => Category::Two,
            3 => Category::Few,
            4 => Category::Many,
            _ => Category::Other,
        }
    }

    /// The keyword a variant key is compared with.
    pub const fn as_str(self) -> &'static str {
        match self {
            Category::Zero => "zero",
            Category::One => "one",
            Category::Two => "two",
            Category::Few => "few",
            Category::Many => "many",
            Category::Other => "other",
        }
    }

    /// The category named `s`.
    pub fn from_keyword(s: &str) -> Option<Category> {
        Some(match s {
            "zero" => Category::Zero,
            "one" => Category::One,
            "two" => Category::Two,
            "few" => Category::Few,
            "many" => Category::Many,
            "other" => Category::Other,
            _ => return None,
        })
    }
}

/// The UTS #35 operands of a non-negative decimal (Part 3 §5.1.1): the
/// formatter's side of the contract of `plans/02-catalog-format.md` §4.1.
/// `n` is integral iff `t == 0`, and then equals `i`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct Operands {
    /// Integer digits.
    pub i: u64,
    /// Visible fraction digits, with trailing zeros.
    pub f: u64,
    /// Visible fraction digits, without trailing zeros.
    pub t: u64,
    /// The number of visible fraction digits, with trailing zeros.
    pub v: u32,
    /// The number of visible fraction digits, without trailing zeros.
    pub w: u32,
    /// The compact decimal exponent (`c`, `e`); 0 from MF2's `:number`.
    pub e: u32,
}

/// Accumulates decimal digits, exact below [`BIG`], else keeping the value
/// modulo 10^18 plus a sticky flag.
#[derive(Clone, Copy, Default)]
pub(crate) struct Acc {
    low: u64,
    big: bool,
}

impl Acc {
    pub(crate) fn push(&mut self, digit: u8) {
        // low < 10^18, so low × 10 + 9 < 2^64.
        let x = self
            .low
            .wrapping_mul(10)
            .wrapping_add(u64::from(digit.min(9)));
        if x >= BIG {
            self.big = true;
            self.low = x % BIG;
        } else {
            self.low = x;
        }
    }

    pub(crate) fn value(self) -> u64 {
        if self.big {
            self.low.wrapping_add(BIG)
        } else {
            self.low
        }
    }
}

impl Operands {
    /// The operands of a CLDR sample or an ASCII decimal:
    /// `-? digit+ ('.' digit+)? ([ce] digit+)?`. The exponent moves the
    /// decimal point right and keeps visible trailing zeros (`1.20050c3` is
    /// `1200.50`, `e = 3`). `None` if `s` does not match.
    pub fn parse(s: &str) -> Option<Operands> {
        let b = s.as_bytes();
        let b = match b.split_first() {
            Some((b'-', rest)) => rest,
            _ => b,
        };
        let (mantissa, exponent) = split_at(b, |c| c == b'c' || c == b'e');
        let e = match exponent {
            None => 0u32,
            Some(x) if all_digits(x) => x.iter().try_fold(0u32, |acc, &c| {
                acc.checked_mul(10)?
                    .checked_add(u32::from(c.wrapping_sub(b'0')))
            })?,
            Some(_) => return None,
        };
        let (int, frac) = split_at(mantissa, |c| c == b'.');
        let frac = match frac {
            None => &[][..],
            Some(x) if all_digits(x) => x,
            Some(_) => return None,
        };
        if !all_digits(int) {
            return None;
        }
        let point = int.len().saturating_add(usize::try_from(e).ok()?);
        let mut ops = OperandsBuilder::default();
        for (k, &c) in int.iter().chain(frac.iter()).enumerate() {
            ops.digit(c.wrapping_sub(b'0'), k < point);
        }
        // An exponent past the digits pads the integer with zeros; after 18
        // zero pushes `low` is 0 and more change nothing, so 19 is exact.
        let total = int.len().saturating_add(frac.len());
        for _ in 0..point.saturating_sub(total).min(19) {
            ops.digit(0, true);
        }
        let mut o = ops.finish();
        o.e = e;
        Some(o)
    }
}

/// Builds operands from digits, most significant first.
#[derive(Default)]
pub(crate) struct OperandsBuilder {
    i: Acc,
    f: Acc,
    t: Acc,
    v: u32,
    w: u32,
    zeros: u32,
}

impl OperandsBuilder {
    /// One digit, of the integer part or of the fraction.
    pub(crate) fn digit(&mut self, d: u8, integer: bool) {
        if integer {
            self.i.push(d);
            return;
        }
        self.f.push(d);
        self.v = self.v.saturating_add(1);
        if d == 0 {
            self.zeros = self.zeros.saturating_add(1);
        } else {
            for _ in 0..self.zeros.min(19) {
                self.t.push(0);
            }
            self.t.push(d);
            self.zeros = 0;
            self.w = self.v;
        }
    }

    pub(crate) fn finish(self) -> Operands {
        Operands {
            i: self.i.value(),
            f: self.f.value(),
            t: self.t.value(),
            v: self.v,
            w: self.w,
            e: 0,
        }
    }
}

fn all_digits(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

fn split_at(s: &[u8], pred: impl Fn(u8) -> bool) -> (&[u8], Option<&[u8]>) {
    match s.iter().position(|&c| pred(c)) {
        Some(p) => match (s.get(..p), s.get(p.wrapping_add(1)..)) {
            (Some(a), Some(b)) => (a, Some(b)),
            _ => (s, None),
        },
        None => (s, None),
    }
}

/// The plural category of `o` under the encoded rules `entry`. An empty
/// entry (every number is `other`) is valid; malformed data gives `other`.
pub fn select(entry: &[u8], o: &Operands) -> Category {
    eval(entry, o).unwrap_or(Category::Other)
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }

    /// Unsigned LEB128, at most 10 bytes.
    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let b = self.byte()?;
            value |= u64::from(b & 0x7f).checked_shl(shift)?;
            if b & 0x80 == 0 {
                return Some(value);
            }
            shift = shift.checked_add(7)?;
        }
    }
}

/// `(value, is_integer)` of an operand. Only `n` can be non-integral.
fn operand(o: &Operands, code: u8) -> Option<(u64, bool)> {
    Some(match code {
        op::N => (o.i, o.t == 0),
        op::I => (o.i, true),
        op::V => (u64::from(o.v), true),
        op::W => (u64::from(o.w), true),
        op::F => (o.f, true),
        op::T => (o.t, true),
        op::E => (u64::from(o.e), true),
        _ => return None,
    })
}

// `n i v w f t e` are UTS #35's operand names.
#[allow(clippy::many_single_char_names)]
fn eval(entry: &[u8], o: &Operands) -> Option<Category> {
    let mut r = Reader(entry);
    while let Some(header) = r.byte() {
        let mut rule = false;
        for _ in 0..(header & RULE_GROUPS_MASK) {
            let mut group = true;
            loop {
                let rel = r.byte()?;
                let (mut x, integral) = operand(o, rel & 7)?;
                let modulus = match (rel >> 3) & 7 {
                    0 => None,
                    MOD_EXPLICIT => Some(r.varint()?),
                    k => {
                        // 1 ≤ k ≤ 6: 10^k cannot wrap (and `checked_pow` would
                        // pull in a 128-bit multiply on wasm32).
                        let mut m = 1u64;
                        for _ in 0..k {
                            m = m.wrapping_mul(10);
                        }
                        Some(m)
                    }
                };
                if let Some(m) = modulus {
                    x = x.checked_rem(m)?;
                }
                let mut hit = false;
                loop {
                    let item = r.varint()?;
                    let lo = item >> 2;
                    let hi = if item & ITEM_RANGE != 0 {
                        lo.checked_add(r.varint()?)?
                    } else {
                        lo
                    };
                    hit |= integral && lo <= x && x <= hi;
                    if item & ITEM_LAST != 0 {
                        break;
                    }
                }
                group &= hit != (rel & REL_NEGATED != 0);
                if rel & REL_LAST != 0 {
                    break;
                }
            }
            rule |= group;
        }
        if rule {
            return Some(Category::from_code(header >> 5));
        }
    }
    Some(Category::Other)
}

#[cfg(test)]
#[allow(clippy::many_single_char_names, clippy::unnecessary_wraps)]
mod tests {
    use super::{BIG, Category, Operands, select};

    fn ops(s: &str) -> Operands {
        Operands::parse(s).unwrap_or_default()
    }

    fn o(i: u64, v: u32, w: u32, f: u64, t: u64, e: u32) -> Option<Operands> {
        Some(Operands { i, f, t, v, w, e })
    }

    /// en cardinal `one: i = 1 and v = 0`.
    const EN: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

    #[test]
    fn en_cardinal() {
        assert_eq!(select(EN, &ops("1")), Category::One);
        assert_eq!(select(EN, &ops("1.0")), Category::Other);
        assert_eq!(select(EN, &ops("2")), Category::Other);
        assert_eq!(select(&[], &ops("1")), Category::Other);
    }

    #[test]
    fn malformed_is_other() {
        for len in 0..EN.len() {
            let _ = select(EN.get(..len).unwrap_or(&[]), &ops("1"));
        }
        assert_eq!(select(&[0x21, 0x07, 0x05], &ops("1")), Category::Other);
        assert_eq!(
            select(&[0x21, 0x39, 0x00, 0x05], &ops("1")),
            Category::Other
        );
        assert_eq!(
            select(&[0x21, 0x01, 0xff, 0xff], &ops("1")),
            Category::Other
        );
        assert_eq!(select(&[0xffu8; 64], &ops("1")), Category::Other);
    }

    /// UTS #35 Part 3 §5.1.1's operand table.
    #[test]
    fn uts35_table() {
        assert_eq!(Operands::parse("1"), o(1, 0, 0, 0, 0, 0));
        assert_eq!(Operands::parse("1.0"), o(1, 1, 0, 0, 0, 0));
        assert_eq!(Operands::parse("1.00"), o(1, 2, 0, 0, 0, 0));
        assert_eq!(Operands::parse("1.3"), o(1, 1, 1, 3, 3, 0));
        assert_eq!(Operands::parse("1.30"), o(1, 2, 1, 30, 3, 0));
        assert_eq!(Operands::parse("1.03"), o(1, 2, 2, 3, 3, 0));
        assert_eq!(Operands::parse("1.230"), o(1, 3, 2, 230, 23, 0));
        assert_eq!(Operands::parse("1200000"), o(1_200_000, 0, 0, 0, 0, 0));
        assert_eq!(Operands::parse("1.2c6"), o(1_200_000, 0, 0, 0, 0, 6));
        assert_eq!(Operands::parse("123c5"), o(12_300_000, 0, 0, 0, 0, 5));
        assert_eq!(Operands::parse("1.20050c3"), o(1200, 2, 1, 50, 5, 3));
        assert_eq!(Operands::parse("1.1e6"), o(1_100_000, 0, 0, 0, 0, 6));
        assert_eq!(Operands::parse("-1.5"), o(1, 1, 1, 5, 5, 0));
        assert_eq!(Operands::parse("0.00"), o(0, 2, 0, 0, 0, 0));
        for bad in ["", "-", ".", "1.", ".5", "1c", "1x", "1..2", "c3"] {
            assert_eq!(Operands::parse(bad), None, "{bad}");
        }
        let x = ops("12345678901234567890123");
        assert_eq!(x.i, BIG + 678_901_234_567_890_123);
    }
}
