//! P0.4 — client-side plural-rule evaluator.
//!
//! Evaluates one encoded `plural.cardinal` / `plural.ordinal` LOCALE entry
//! (encoding: `RESULT.md` §"Byte encoding", produced by `plural-rules`) against
//! the UTS #35 plural operands of a number, and returns the plural category.
//!
//! Client-path rules: `no_std`, no allocation, `forbid(unsafe_code)`, no
//! `core::fmt` use, no panicking operation (every read is a checked `split_first`,
//! every arithmetic step is checked or provably in range). Malformed data never
//! panics: evaluation stops and the category is `other`, the spec's catch-all.
#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]
// `i v w f t e n` are the UTS #35 operand names.
#![allow(clippy::many_single_char_names)]

mod operands;

pub use operands::Operands;

/// Operand codes (relation header bits 0–2). `c` and `e` share code 6 (UTS #35:
/// `e` is a synonym of `c`). Code 7 is invalid.
pub mod op {
    pub const N: u8 = 0;
    pub const I: u8 = 1;
    pub const V: u8 = 2;
    pub const W: u8 = 3;
    pub const F: u8 = 4;
    pub const T: u8 = 5;
    pub const E: u8 = 6;
}

/// Modulus field (relation header bits 3–5): 0 = none, 1..=6 = `% 10^k`,
/// 7 = an explicit LEB128 modulus follows the header.
pub const MOD_EXPLICIT: u8 = 7;
/// Relation header bit 6: the relation is `!=` (negated).
pub const REL_NEGATED: u8 = 0x40;
/// Relation header bit 7: last relation of its AND group.
pub const REL_LAST: u8 = 0x80;
/// Range item bit 0: last item of the range list.
pub const ITEM_LAST: u64 = 1;
/// Range item bit 1: the item is a range; a LEB128 `hi - lo` follows.
pub const ITEM_RANGE: u64 = 2;
/// Rule header: bits 5–7 category code, bits 0–4 number of OR groups.
pub const RULE_GROUPS_MASK: u8 = 0x1f;

/// CLDR plural category.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Category {
    Zero = 0,
    One = 1,
    Two = 2,
    Few = 3,
    Many = 4,
    Other = 5,
}

impl Category {
    /// Category for a rule-header code; codes ≥ 5 mean `other`.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Zero,
            1 => Self::One,
            2 => Self::Two,
            3 => Self::Few,
            4 => Self::Many,
            _ => Self::Other,
        }
    }

    /// The MF2 / CLDR keyword (`"one"`, …) — what a variant key is compared with.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// Selects the plural category of `o` under the encoded rules `entry`.
///
/// An empty entry (every number is `other`, e.g. `ja` cardinal) is valid.
/// Malformed data yields [`Category::Other`].
#[must_use]
pub fn select(entry: &[u8], o: &Operands) -> Category {
    match eval(entry, o) {
        Some(c) => c,
        None => Category::Other,
    }
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

/// `(value, is_integer)` for an operand code. Only `n` can be a non-integer:
/// `n` is integral exactly when it has no non-zero fraction digit (`t == 0`), and
/// then `n % m == i % m`. A non-integral `n` is in no value list or range.
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
                        // 1 <= k <= 6: 10^k cannot wrap. (checked_pow would pull
                        // in a 128-bit multiply helper for its overflow test.)
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
// No `Debug` on client types, hence `assert!(a == b)`.
#[allow(clippy::panic, clippy::pedantic)]
mod tests {
    use super::{Category, Operands, select};

    fn ops(s: &str) -> Operands {
        Operands::parse(s.as_bytes()).unwrap_or_default()
    }

    // en cardinal "one: i = 1 and v = 0":
    // rule 0x21 (one, 1 group) · rel i (0x01) item 1 last (0x05) · rel v last-in-group (0x82) item 0 last (0x01)
    const EN: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

    #[test]
    fn en_cardinal() {
        assert!(select(EN, &ops("1")) == Category::One);
        assert!(select(EN, &ops("1.0")) == Category::Other);
        assert!(select(EN, &ops("2")) == Category::Other);
        assert!(select(EN, &ops("0")) == Category::Other);
    }

    #[test]
    fn empty_entry_is_other() {
        assert!(select(&[], &ops("1")) == Category::Other);
    }

    #[test]
    fn malformed_is_other_never_panics() {
        for len in 0..EN.len() {
            let _ = select(EN.get(..len).unwrap_or(&[]), &ops("1"));
        }
        assert!(select(&[0x21, 0x07, 0x05], &ops("1")) == Category::Other); // operand code 7
        assert!(select(&[0x21, 0x39, 0x00, 0x05], &ops("1")) == Category::Other); // explicit modulus 0
        assert!(select(&[0x21, 0x01, 0xff, 0xff], &ops("1")) == Category::Other); // unterminated varint
        let long = [0xffu8; 64];
        assert!(select(&long, &ops("1")) == Category::Other);
    }
}
