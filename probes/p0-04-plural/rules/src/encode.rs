//! Encoder for the `plural.cardinal` / `plural.ordinal` LOCALE entries.
//! The byte format is specified in RESULT.md ("Byte encoding"); the constants
//! live in `plural-eval` so the reader and the writer cannot drift.
//!
//! Canonical form (so the output is deterministic, 02 F8): rules in category
//! order zero < one < two < few < many, `other` never encoded; OR groups,
//! relations and range items in source order; minimal LEB128; a range with
//! `lo == hi` is written as a single value; a modulus 10^k (1 ≤ k ≤ 6) uses the
//! short code k, any other modulus the explicit form.

use plural_eval::{Category, ITEM_LAST, ITEM_RANGE, MOD_EXPLICIT, REL_LAST, REL_NEGATED, op};

use crate::rule::{Operand, Relation, Rule};

fn leb128(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7f) as u8;
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
        Operand::N => op::N,
        Operand::I => op::I,
        Operand::V => op::V,
        Operand::W => op::W,
        Operand::F => op::F,
        Operand::T => op::T,
        Operand::C => op::E,
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

/// Appends one rule (header + OR groups). `other` rules are skipped.
pub fn encode_rule(rule: &Rule, out: &mut Vec<u8>) {
    if rule.category == Category::Other || rule.condition.is_empty() {
        return;
    }
    // parse_rule guarantees ≤ 31 groups.
    out.push(((rule.category as u8) << 5) | (rule.condition.len() as u8));
    for group in &rule.condition {
        for (k, r) in group.iter().enumerate() {
            relation(r, k + 1 == group.len(), out);
        }
    }
}

/// Encodes a locale's rule set into one LOCALE entry (canonical order).
pub fn encode(rules: &[Rule]) -> Vec<u8> {
    let mut sorted: Vec<&Rule> = rules.iter().collect();
    sorted.sort_by_key(|r| r.category as u8);
    let mut out = Vec::new();
    for r in sorted {
        encode_rule(r, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::parse_rule;

    #[test]
    fn en_cardinal_bytes() {
        let rules = [
            parse_rule(Category::One, "i = 1 and v = 0 @integer 1").unwrap(),
            parse_rule(Category::Other, " @integer 0, 2~16").unwrap(),
        ];
        assert_eq!(encode(&rules), [0x21, 0x01, 0x05, 0x82, 0x01]);
    }

    #[test]
    fn modulus_ranges_and_explicit_modulus() {
        let r = parse_rule(Category::Few, "n % 100 = 3..10 or n % 7 != 1,2").unwrap();
        let mut out = Vec::new();
        encode_rule(&r, &mut out);
        // few|2 groups; n%10^2 last: item 3 range (3<<2|2 = 14), hi-lo 7 → but last → 15, 7
        // n%explicit negated last: 0x80|0x40|0x38 = 0xF8, modulus 7, items 1 (0x04), 2 last (0x09)
        assert_eq!(out, [0x62, 0x90, 0x0f, 0x07, 0xf8, 0x07, 0x04, 0x09]);
    }

    #[test]
    fn leb128_multi_byte() {
        let mut out = Vec::new();
        leb128(1_000_000 << 2, &mut out);
        assert_eq!(out, [0x80, 0x92, 0xf4, 0x01]);
    }
}
