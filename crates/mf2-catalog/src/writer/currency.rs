//! The canonical encoder of `currency.data` (feature `writer`;
//! `plans/02-catalog-format.md` §4.6). [`crate::currency`] reads it.

use alloc::vec::Vec;

use crate::currency::{
    E_DECIMAL, E_GROUP, E_PATTERN, F_NAMES, F_NARROW, H_NAME, H_NARROW, H_ROUNDING, H_SYMBOL,
};
use crate::error::WriteError;
use crate::format::locale_key;
use crate::number::{OTHER, TemplatePart};
use crate::writer::encode::varint;
use crate::writer::number::{PatternSpec, body, forms, str8, template};

/// One currency.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CurrencySpec<'s> {
    /// The ISO 4217 code, three upper-case ASCII letters.
    pub code: &'s str,
    /// The fraction digits (0–15).
    pub digits: u8,
    /// The rounding increment (0 = none).
    pub rounding: u32,
    /// The symbol (the code when CLDR has none).
    pub symbol: &'s str,
    /// The narrow symbol (the symbol when CLDR has none).
    pub narrow: &'s str,
    /// The display name (the code when CLDR has none).
    pub name: &'s str,
    /// The display names by plural category (`displayName-count-*`: codes 0
    /// zero … 5 other), as CLDR has them.
    pub names: Vec<(u8, &'s str)>,
    /// [`crate::currency::Edges`] of the symbol (bits 0, 1) and of the narrow
    /// symbol (bits 2, 3), computed by the caller (`General_Category`).
    pub edges: u8,
    /// The currency's own standard pattern (CLDR `currencies/*/pattern`).
    pub pattern: Option<PatternSpec<'s>>,
    /// The currency's own decimal separator.
    pub decimal: Option<&'s str>,
    /// The currency's own group separator.
    pub group: Option<&'s str>,
}

/// A `currency.data` entry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CurrenciesSpec<'s> {
    /// CLDR's `fractions/DEFAULT` digits.
    pub default_digits: u8,
    /// Carry narrow symbols.
    pub narrow: bool,
    /// Carry display names and the name patterns.
    pub names: bool,
    /// The name patterns by plural category (`{0}` number, `{1}` name).
    pub name_patterns: Vec<(u8, Vec<TemplatePart<'s>>)>,
    /// The currencies, in any order.
    pub currencies: Vec<CurrencySpec<'s>>,
}

const ERR: WriteError = WriteError::LocaleEntry(locale_key::CURRENCY_DATA);

/// The display-name forms, canonical: `other` dropped when it equals the
/// name; any other category dropped when it equals what the reader's
/// fallback gives (`other`'s form, else the name).
fn name_forms<'s>(name: &'s str, list: &[(u8, &'s str)]) -> Vec<(u8, &'s str)> {
    let other = list.iter().find(|(c, _)| *c == OTHER).map(|(_, s)| *s);
    let other = other.filter(|o| *o != name);
    let fallback = other.unwrap_or(name);
    let mut out: Vec<(u8, &str)> = list
        .iter()
        .filter(|(c, s)| {
            if *c == OTHER {
                other.is_some()
            } else {
                *s != fallback
            }
        })
        .copied()
        .collect();
    out.sort_by_key(|(c, _)| *c);
    out
}

/// The `currency.data` payload (§4.6). Canonical: currencies sorted by
/// code; a symbol equal to the code, a narrow symbol equal to the symbol,
/// a name equal to the code are not stored; a display-name form equal to what
/// the reader's fallback gives is dropped;
/// a name pattern equal to `other`'s is dropped.
/// Refuses a malformed code, a code twice, digits over 15, a string over
/// 255 bytes or with a byte below 0x20.
pub fn currencies(spec: &CurrenciesSpec<'_>) -> Result<Vec<u8>, WriteError> {
    let mut list: Vec<&CurrencySpec<'_>> = spec.currencies.iter().collect();
    list.sort_by_key(|c| c.code);
    if list
        .windows(2)
        .any(|w| matches!(w, [a, b] if a.code == b.code))
    {
        return Err(ERR);
    }
    let flags = (u8::from(spec.narrow) * F_NARROW) | (u8::from(spec.names) * F_NAMES);
    let mut out = Vec::new();
    out.push(flags);
    out.push(spec.default_digits);
    if spec.names {
        // A category whose pattern equals `other`'s is left to the fallback.
        let other = spec
            .name_patterns
            .iter()
            .find(|(c, _)| *c == OTHER)
            .map(|(_, t)| t);
        let pats: Vec<(u8, &[TemplatePart<'_>])> = spec
            .name_patterns
            .iter()
            .filter(|(c, t)| *c == OTHER || Some(t) != other)
            .map(|(c, t)| (*c, t.as_slice()))
            .collect();
        forms(&pats, &mut out, &ERR, |parts, out| {
            template(parts, out, &ERR)
        })?;
    }
    let n = u16::try_from(list.len()).map_err(|_| ERR)?;
    out.extend_from_slice(&n.to_le_bytes());
    let mut records = Vec::new();
    let mut index = Vec::with_capacity(list.len() * 7);
    for c in list {
        if c.code.len() != 3 || !c.code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(ERR);
        }
        if c.digits > 15 || c.edges > 0x0f {
            return Err(ERR);
        }
        index.extend_from_slice(c.code.as_bytes());
        index.extend_from_slice(&u32::try_from(records.len()).map_err(|_| ERR)?.to_le_bytes());
        let symbol = (c.symbol != c.code).then_some(c.symbol);
        let narrow = (spec.narrow && c.narrow != c.symbol).then_some(c.narrow);
        let name = (spec.names && c.name != c.code).then_some(c.name);
        let mut head = c.digits;
        if c.rounding != 0 {
            head |= H_ROUNDING;
        }
        if symbol.is_some() {
            head |= H_SYMBOL;
        }
        if narrow.is_some() {
            head |= H_NARROW;
        }
        if name.is_some() {
            head |= H_NAME;
        }
        records.push(head);
        if c.rounding != 0 {
            varint(c.rounding, &mut records);
        }
        let mut edges = if spec.narrow { c.edges } else { c.edges & 0x03 };
        if c.pattern.is_some() {
            edges |= E_PATTERN;
        }
        if c.decimal.is_some() {
            edges |= E_DECIMAL;
        }
        if c.group.is_some() {
            edges |= E_GROUP;
        }
        records.push(edges);
        for s in [symbol, narrow].into_iter().flatten() {
            str8(s, &mut records, &ERR)?;
        }
        if let Some(p) = &c.pattern {
            let b = body(p)?;
            records.push(u8::try_from(b.len()).map_err(|_| ERR)?);
            records.extend_from_slice(&b);
        }
        for s in [c.decimal, c.group, name].into_iter().flatten() {
            str8(s, &mut records, &ERR)?;
        }
        if spec.names {
            let list = name_forms(c.name, &c.names);
            forms(&list, &mut records, &ERR, |s, out| str8(s, out, &ERR))?;
        }
    }
    out.extend_from_slice(&index);
    out.extend_from_slice(&records);
    Ok(out)
}
