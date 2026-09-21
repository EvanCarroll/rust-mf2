//! Structural check of a `plural.cardinal` / `plural.ordinal` LOCALE entry
//! (`plans/02-catalog-format.md` §4.1), done once by `Catalog::new` (F4).
//! The evaluator (Phase 3) still answers `other` on malformed data; this walk
//! only guarantees that a loaded catalog has none.

use crate::bytes::{Cur, varint64};

/// Rule header bits 0–4: the number of OR groups.
const GROUPS: u8 = 0x1f;
/// Relation header bit 7: last relation of its OR group.
const LAST: u8 = 0x80;
/// Relation header bits 3–5 value: an explicit modulus follows.
const MOD_EXPLICIT: u8 = 7;
/// Item bit 0: last item of the relation's list.
const ITEM_LAST: u64 = 1;
/// Item bit 1: a range; `hi − lo` follows.
const ITEM_RANGE: u64 = 2;

/// Whether `entry` is a well-formed v1 plural entry. Linear in its length.
pub(crate) fn valid(entry: &[u8]) -> bool {
    walk(entry).is_some()
}

fn walk(entry: &[u8]) -> Option<()> {
    let mut c = Cur::new(entry, 0);
    while !c.at_end() {
        let rule = c.u8()?;
        // Categories 0..=4 (zero … many); 5..=7 reserved. 1..=31 OR groups.
        if rule >> 5 > 4 || rule & GROUPS == 0 {
            return None;
        }
        for _ in 0..rule & GROUPS {
            loop {
                let rel = c.u8()?;
                if rel & 7 == 7 {
                    return None;
                }
                if (rel >> 3) & 7 == MOD_EXPLICIT && varint64(&mut c)? == 0 {
                    return None;
                }
                loop {
                    let item = varint64(&mut c)?;
                    if item & ITEM_RANGE != 0 {
                        let d = varint64(&mut c)?;
                        if d == 0 {
                            return None;
                        }
                        (item >> 2).checked_add(d)?;
                    }
                    if item & ITEM_LAST != 0 {
                        break;
                    }
                }
                if rel & LAST != 0 {
                    break;
                }
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::valid;

    #[test]
    fn entries() {
        // en cardinal: one: i = 1 and v = 0.
        assert!(valid(&[0x21, 0x01, 0x05, 0x82, 0x01]));
        assert!(valid(&[]));
        let en = [0x21, 0x01, 0x05, 0x82, 0x01];
        for len in 1..5 {
            assert!(!valid(en.get(..len).unwrap_or(&[0xff])));
        }
        assert!(!valid(&[0x20])); // zero groups
        assert!(!valid(&[0xa1, 0x81, 0x01])); // category 5
        assert!(!valid(&[0x21, 0x87, 0x01])); // operand 7
        assert!(!valid(&[0x21, 0xb9, 0x00, 0x01])); // explicit modulus 0
        assert!(!valid(&[0x21, 0x81, 0x03, 0x00])); // range of width 0
        assert!(valid(&[0x21, 0x81, 0x07, 0x05])); // i in 1..6
    }
}
