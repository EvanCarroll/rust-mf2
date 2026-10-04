//! The canonical encoder of the `plural.cardinal` / `plural.ordinal` LOCALE
//! entries (ported from P0.4): rules in
//! category order zero < one < two < few < many, `other` never written;
//! groups, relations and items in source order; minimal LEB128; a range with
//! `lo = hi` as a single value; moduli 10^1…10^6 as the short code.

use super::rule::{Category, Operand, Relation, Rule};

/// Relation header bit 6: `!=`.
const REL_NEGATED: u8 = 0x40;
/// Relation header bit 7: last relation of its group.
const REL_LAST: u8 = 0x80;
/// Modulus field: an explicit LEB128 modulus follows.
const MOD_EXPLICIT: u8 = 7;
/// Item bit 0: last item of the relation's list.
const ITEM_LAST: u64 = 1;
/// Item bit 1: a range; `hi − lo` follows.
const ITEM_RANGE: u64 = 2;

fn leb128(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(v & 0x7f).unwrap_or(0);
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn operand_code(o: Operand) -> u8 {
    match o {
        Operand::N => 0,
        Operand::I => 1,
        Operand::V => 2,
        Operand::W => 3,
        Operand::F => 4,
        Operand::T => 5,
        Operand::C => 6,
    }
}

fn modulus_code(m: Option<u64>) -> (u8, Option<u64>) {
    match m {
        None => (0, None),
        Some(m) => match (1u8..=6).find(|&k| 10u64.pow(u32::from(k)) == m) {
            Some(k) => (k, None),
            None => (MOD_EXPLICIT, Some(m)),
        },
    }
}

fn relation(r: &Relation, last: bool, out: &mut Vec<u8>) {
    let (mcode, explicit) = modulus_code(r.modulus);
    let mut header = operand_code(r.operand) | (mcode << 3);
    if r.negated {
        header |= REL_NEGATED;
    }
    if last {
        header |= REL_LAST;
    }
    out.push(header);
    if let Some(m) = explicit {
        leb128(m, out);
    }
    for (k, &(lo, hi)) in r.items.iter().enumerate() {
        let mut item = lo << 2;
        if hi != lo {
            item |= ITEM_RANGE;
        }
        if k + 1 == r.items.len() {
            item |= ITEM_LAST;
        }
        leb128(item, out);
        if hi != lo {
            leb128(hi - lo, out);
        }
    }
}

/// Appends one rule (header and groups); `other` writes nothing.
pub fn encode_rule(rule: &Rule, out: &mut Vec<u8>) {
    if rule.category == Category::Other || rule.condition.is_empty() {
        return;
    }
    // `parse_rule` allows at most 31 groups.
    let groups = u8::try_from(rule.condition.len()).unwrap_or(31);
    out.push(((rule.category as u8) << 5) | groups);
    for group in &rule.condition {
        for (k, r) in group.iter().enumerate() {
            relation(r, k + 1 == group.len(), out);
        }
    }
}

/// One LOCALE entry for a locale's rules, canonical.
pub fn encode(rules: &[Rule]) -> Vec<u8> {
    let mut sorted: Vec<&Rule> = rules.iter().collect();
    sorted.sort_by_key(|r| r.category);
    let mut out = Vec::new();
    for r in sorted {
        encode_rule(r, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{encode, encode_rule, leb128};
    use crate::plural::rule::{Category, parse_rule};

    #[test]
    fn en_cardinal_bytes() {
        let rules = [
            parse_rule(Category::One, "i = 1 and v = 0 @integer 1").expect("one"),
            parse_rule(Category::Other, " @integer 0, 2~16").expect("other"),
        ];
        assert_eq!(encode(&rules), [0x21, 0x01, 0x05, 0x82, 0x01]);
    }

    #[test]
    fn modulus_ranges_explicit_modulus() {
        let r = parse_rule(Category::Few, "n % 100 = 3..10 or n % 7 != 1,2").expect("few");
        let mut out = Vec::new();
        encode_rule(&r, &mut out);
        assert_eq!(out, [0x62, 0x90, 0x0f, 0x07, 0xf8, 0x07, 0x04, 0x09]);
    }

    #[test]
    fn multi_byte_leb128() {
        let mut out = Vec::new();
        leb128(1_000_000 << 2, &mut out);
        assert_eq!(out, [0x80, 0x92, 0xf4, 0x01]);
    }
}
