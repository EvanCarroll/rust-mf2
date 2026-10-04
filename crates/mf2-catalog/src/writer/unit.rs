//! The canonical encoder of `unit.data` (feature `writer`).
//! [`crate::unit`] reads it.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::WriteError;
use crate::format::locale_key;
use crate::number::{OTHER, TemplatePart};
use crate::unit::{B_NAME, B_PER, B_SAME, F_NAMES, Width};
use crate::writer::number::{forms, str8, template};

/// One width of one unit, as CLDR has it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct UnitWidthSpec<'s> {
    /// `displayName`.
    pub name: Option<&'s str>,
    /// `perUnitPattern` (`{0}` the numerator).
    pub per_unit: Option<Vec<TemplatePart<'s>>>,
    /// `unitPattern-count-*` by plural category (codes 0–5; `{0}` the
    /// number). Empty when CLDR has none in this width; else it has `other`.
    pub patterns: Vec<(u8, Vec<TemplatePart<'s>>)>,
}

/// One unit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnitSpec<'s> {
    /// The identifier without its category (`kilometer-per-hour`).
    pub id: &'s str,
    /// One block per carried width, in the order of [`UnitsSpec::widths`].
    pub widths: Vec<UnitWidthSpec<'s>>,
}

/// A `unit.data` entry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnitsSpec<'s> {
    /// The widths carried, in any order (each once).
    pub widths: Vec<Width>,
    /// Carry display names.
    pub names: bool,
    /// The locale's `per` compound pattern, one per carried width, in the
    /// order of `widths`.
    pub per: Vec<Vec<TemplatePart<'s>>>,
    /// The units, in any order.
    pub units: Vec<UnitSpec<'s>>,
}

const ERR: WriteError = WriteError::LocaleEntry(locale_key::UNIT_DATA);

fn block(b: &UnitWidthSpec<'_>, names: bool) -> Result<Vec<u8>, WriteError> {
    let other = b.patterns.iter().find(|(c, _)| *c == OTHER).map(|(_, p)| p);
    if !b.patterns.is_empty() && other.is_none() {
        return Err(ERR);
    }
    let name = b.name.filter(|_| names);
    let mut head = 0u8;
    if name.is_some() {
        head |= B_NAME;
    }
    if b.per_unit.is_some() {
        head |= B_PER;
    }
    let mut out = vec![head];
    if let Some(n) = name {
        str8(n, &mut out, &ERR)?;
    }
    if let Some(p) = &b.per_unit {
        template(p, &mut out, &ERR)?;
    }
    // A category whose pattern equals `other`'s is left to the fallback.
    let kept: Vec<(u8, &[TemplatePart<'_>])> = b
        .patterns
        .iter()
        .filter(|(c, p)| *c == OTHER || Some(p) != other)
        .map(|(c, p)| (*c, p.as_slice()))
        .collect();
    forms(&kept, &mut out, &ERR, |p, out| template(p, out, &ERR))?;
    Ok(out)
}

/// The `unit.data` payload (§4.7). Canonical: widths in the order long,
/// short, narrow; units sorted by identifier (bytewise); a block equal to
/// the previous width's is written as one byte; a pattern equal to `other`'s
/// is dropped. Refuses an empty or duplicate identifier, a width count that
/// does not match, patterns without `other`, over-long strings.
pub fn units(spec: &UnitsSpec<'_>) -> Result<Vec<u8>, WriteError> {
    let mut widths = spec.widths.clone();
    widths.sort_unstable();
    widths.dedup();
    if widths.len() != spec.widths.len() || spec.per.len() != widths.len() {
        return Err(ERR);
    }
    let mut flags = widths.iter().fold(0u8, |f, w| f | w.bit());
    if spec.names {
        flags |= F_NAMES;
    }
    let mut out = vec![flags];
    // `per` in the caller's width order, written in canonical order.
    for w in &widths {
        let at = spec.widths.iter().position(|x| x == w).ok_or(ERR)?;
        template(spec.per.get(at).ok_or(ERR)?, &mut out, &ERR)?;
    }
    let mut list: Vec<&UnitSpec<'_>> = spec.units.iter().collect();
    list.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
    if list.windows(2).any(|w| matches!(w, [a, b] if a.id == b.id)) {
        return Err(ERR);
    }
    out.extend_from_slice(&u16::try_from(list.len()).map_err(|_| ERR)?.to_le_bytes());
    let mut index = Vec::with_capacity(list.len() * 4);
    let mut records = Vec::new();
    for u in list {
        if u.id.is_empty() || u.widths.len() != widths.len() {
            return Err(ERR);
        }
        index.extend_from_slice(&u32::try_from(records.len()).map_err(|_| ERR)?.to_le_bytes());
        str8(u.id, &mut records, &ERR)?;
        let mut prev: Option<Vec<u8>> = None;
        for w in &widths {
            let at = spec.widths.iter().position(|x| x == w).ok_or(ERR)?;
            let b = block(u.widths.get(at).ok_or(ERR)?, spec.names)?;
            if prev.as_ref() == Some(&b) {
                records.push(B_SAME);
            } else {
                records.extend_from_slice(&b);
            }
            prev = Some(b);
        }
    }
    out.extend_from_slice(&index);
    out.extend_from_slice(&records);
    Ok(out)
}
