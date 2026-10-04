//! The canonical encoders of the number LOCALE entries (feature `writer`):
//! [`symbols`] writes a
//! `number.symbols` payload, [`patterns`] a `number.patterns` payload. Both
//! validate their input against the format's limits and give the same bytes
//! for the same input (F8); [`crate::number`] reads them.

use alloc::vec::Vec;

use crate::error::WriteError;
use crate::format::locale_key;
use crate::number::{
    AffixPart, Grouping, OTHER, PH_CURRENCY, PH_PERCENT, PH_SIGN, Style, T_ARG0, T_ARG1,
    TemplatePart,
};

/// The content of a `number.symbols` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SymbolsSpec<'s> {
    /// The decimal pattern's grouping (sizes ≤ 15).
    pub grouping: Grouping,
    /// CLDR's `minimumGroupingDigits` (1–15).
    pub minimum_grouping_digits: u8,
    pub decimal: &'s str,
    pub group: &'s str,
    pub minus: &'s str,
    pub plus: &'s str,
    pub percent: &'s str,
    /// The numbering system's ten digits in order, each of the same UTF-8
    /// width; `None` (or the ASCII digits) for `latn`.
    pub digits: Option<&'s str>,
}

/// One pattern of a `number.patterns` entry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PatternSpec<'s> {
    /// The pattern's grouping (sizes ≤ 15).
    pub grouping: Grouping,
    /// The positive subpattern's prefix and suffix.
    pub positive: (Vec<AffixPart<'s>>, Vec<AffixPart<'s>>),
    /// The explicit negative subpattern's prefix and suffix, if any.
    pub negative: Option<(Vec<AffixPart<'s>>, Vec<AffixPart<'s>>)>,
}

const SYMBOLS: WriteError = WriteError::LocaleEntry(locale_key::NUMBER_SYMBOLS);
const PATTERNS: WriteError = WriteError::LocaleEntry(locale_key::NUMBER_PATTERNS);

fn str8_any(s: &str, out: &mut Vec<u8>, err: &WriteError) -> Result<(), WriteError> {
    let n = u8::try_from(s.len()).map_err(|_| err.clone())?;
    out.push(n);
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

/// `u8 len · UTF-8{len}` of text without bytes below 0x20.
pub(crate) fn str8(s: &str, out: &mut Vec<u8>, err: &WriteError) -> Result<(), WriteError> {
    if s.bytes().any(|b| b < 0x20) {
        return Err(err.clone());
    }
    str8_any(s, out, err)
}

/// A template (§4.6): `str8` with `{0}` as 0x01 and `{1}` as 0x02,
/// adjacent text merged.
pub(crate) fn template(
    parts: &[TemplatePart<'_>],
    out: &mut Vec<u8>,
    err: &WriteError,
) -> Result<(), WriteError> {
    let mut bytes = Vec::new();
    for p in parts {
        match *p {
            TemplatePart::Text(t) => {
                if t.bytes().any(|b| b < 0x20) {
                    return Err(err.clone());
                }
                bytes.extend_from_slice(t.as_bytes());
            }
            TemplatePart::Arg0 => bytes.push(T_ARG0),
            TemplatePart::Arg1 => bytes.push(T_ARG1),
        }
    }
    out.push(u8::try_from(bytes.len()).map_err(|_| err.clone())?);
    out.extend_from_slice(&bytes);
    Ok(())
}

/// Plural forms (§4.6): `u8 k · (u8 category · value){k}`, categories
/// (0–5) sorted; a category twice is refused.
pub(crate) fn forms<T>(
    list: &[(u8, T)],
    out: &mut Vec<u8>,
    err: &WriteError,
    mut value: impl FnMut(&T, &mut Vec<u8>) -> Result<(), WriteError>,
) -> Result<(), WriteError> {
    let mut sorted: Vec<&(u8, T)> = list.iter().collect();
    sorted.sort_by_key(|(c, _)| *c);
    if sorted.windows(2).any(|w| matches!(w, [a, b] if a.0 == b.0))
        || sorted.iter().any(|(c, _)| *c > OTHER)
    {
        return Err(err.clone());
    }
    out.push(u8::try_from(sorted.len()).map_err(|_| err.clone())?);
    for (c, v) in sorted {
        out.push(*c);
        value(v, out)?;
    }
    Ok(())
}

/// The `number.symbols` payload (§4.2): `u8 grouping · u8 min_grouping ·
/// str8 decimal · str8 group · str8 minus · str8 plus · str8 percent ·
/// digits`, digits empty for ASCII.
pub fn symbols(s: &SymbolsSpec<'_>) -> Result<Vec<u8>, WriteError> {
    let grouping = s.grouping.to_byte().ok_or(SYMBOLS)?;
    if !(1..=15).contains(&s.minimum_grouping_digits) {
        return Err(SYMBOLS);
    }
    let mut out = Vec::new();
    out.push(grouping);
    out.push(s.minimum_grouping_digits);
    for text in [s.decimal, s.group, s.minus, s.plus, s.percent] {
        str8_any(text, &mut out, &SYMBOLS)?;
    }
    match s.digits {
        None | Some("0123456789") => {}
        Some(d) => {
            let mut widths = d.chars().map(char::len_utf8);
            let w = widths.next().ok_or(SYMBOLS)?;
            if d.chars().count() != 10 || widths.any(|x| x != w) {
                return Err(SYMBOLS);
            }
            out.extend_from_slice(d.as_bytes());
        }
    }
    Ok(out)
}

/// Appends one affix: adjacent text merged, placeholders as their bytes.
fn affix(parts: &[AffixPart<'_>], out: &mut Vec<u8>) -> Result<(), WriteError> {
    let mut bytes = Vec::new();
    for p in parts {
        match *p {
            AffixPart::Text(t) => {
                if t.bytes().any(|b| b < 0x20) {
                    return Err(PATTERNS);
                }
                bytes.extend_from_slice(t.as_bytes());
            }
            AffixPart::Sign => bytes.push(PH_SIGN),
            AffixPart::Percent => bytes.push(PH_PERCENT),
            AffixPart::Currency => bytes.push(PH_CURRENCY),
        }
    }
    out.push(u8::try_from(bytes.len()).map_err(|_| PATTERNS)?);
    out.extend_from_slice(&bytes);
    Ok(())
}

/// One record body: `u8 grouping · affix · affix · [affix · affix]`.
pub(crate) fn body(p: &PatternSpec<'_>) -> Result<Vec<u8>, WriteError> {
    let mut out = Vec::new();
    out.push(p.grouping.to_byte().ok_or(PATTERNS)?);
    affix(&p.positive.0, &mut out)?;
    affix(&p.positive.1, &mut out)?;
    if let Some((prefix, suffix)) = &p.negative {
        affix(prefix, &mut out)?;
        affix(suffix, &mut out)?;
    }
    Ok(out)
}

/// The `number.patterns` payload (§4.3): `(u8 style · u8 len · body)*`,
/// styles strictly ascending. Canonical: records sorted by style; an
/// `…Alpha` record whose bytes equal its base's is omitted (a reader falls
/// back to the base, [`crate::number::Patterns::resolve`]). A style given
/// twice is refused.
pub fn patterns(list: &[(Style, PatternSpec<'_>)]) -> Result<Vec<u8>, WriteError> {
    let mut records: Vec<(Style, Vec<u8>)> = Vec::with_capacity(list.len());
    for (style, p) in list {
        records.push((*style, body(p)?));
    }
    records.sort_by_key(|r| r.0);
    if records
        .windows(2)
        .any(|w| matches!(w, [a, b] if a.0 == b.0))
    {
        return Err(PATTERNS);
    }
    let mut out = Vec::new();
    for (style, bytes) in &records {
        let same_as_base = style.base().is_some_and(|b| {
            records
                .iter()
                .any(|(s, base_bytes)| *s == b && base_bytes == bytes)
        });
        if same_as_base {
            continue;
        }
        out.push(*style as u8);
        out.push(u8::try_from(bytes.len()).map_err(|_| PATTERNS)?);
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use alloc::vec;

    use super::{PatternSpec, SymbolsSpec, patterns, symbols};
    use crate::number::{AffixPart, Grouping, Patterns, Style, Symbols};

    const WEST: Grouping = Grouping {
        primary: 3,
        secondary: 3,
    };

    #[test]
    fn symbols_round_trip_and_limits() {
        let mut s = SymbolsSpec {
            grouping: WEST,
            minimum_grouping_digits: 1,
            decimal: "٫",
            group: "٬",
            minus: "\u{61c}-",
            plus: "\u{61c}+",
            percent: "٪\u{61c}",
            digits: Some("٠١٢٣٤٥٦٧٨٩"),
        };
        let bytes = symbols(&s).expect("encodes");
        let v = Symbols::parse(&bytes).expect("parses");
        assert_eq!(v.decimal(), "٫");
        assert_eq!(v.digits().digit(3), "٣");
        assert_eq!(v.digits().as_str(), "٠١٢٣٤٥٦٧٨٩");
        s.digits = Some("0123456789");
        assert!(
            Symbols::parse(&symbols(&s).expect("ascii"))
                .expect("parses")
                .digits()
                .is_ascii()
        );
        s.digits = Some("0١٢٣٤٥٦٧٨٩"); // mixed widths
        assert!(symbols(&s).is_err());
        s.digits = None;
        s.minimum_grouping_digits = 0;
        assert!(symbols(&s).is_err());
        s.minimum_grouping_digits = 1;
        s.grouping.primary = 16;
        assert!(symbols(&s).is_err());
    }

    #[test]
    fn patterns_canonical() {
        let std = PatternSpec {
            grouping: WEST,
            positive: (vec![AffixPart::Currency], vec![]),
            negative: None,
        };
        let spaced = PatternSpec {
            grouping: WEST,
            positive: (vec![AffixPart::Currency, AffixPart::Text("\u{a0}")], vec![]),
            negative: None,
        };
        // Given out of order, with an …Alpha equal to its base: sorted, omitted.
        let a = patterns(&[
            (Style::AccountingAlpha, std.clone()),
            (Style::Currency, std.clone()),
            (Style::CurrencyAlpha, spaced.clone()),
            (Style::Accounting, std.clone()),
        ])
        .expect("encodes");
        let p = Patterns::new(&a);
        assert!(p.is_valid());
        assert_eq!(
            p.styles().collect::<alloc::vec::Vec<_>>(),
            [Style::Currency, Style::CurrencyAlpha, Style::Accounting]
        );
        assert_eq!(p.resolve(Style::AccountingAlpha), p.get(Style::Accounting));
        assert!(patterns(&[(Style::Currency, std.clone()), (Style::Currency, spaced)]).is_err());
        let bad = PatternSpec {
            grouping: WEST,
            positive: (vec![AffixPart::Text("\u{1}")], vec![]),
            negative: None,
        };
        assert!(patterns(&[(Style::Percent, bad)]).is_err());
        // Adjacent text merges: one record either way.
        let split = PatternSpec {
            grouping: WEST,
            positive: (vec![AffixPart::Text("a"), AffixPart::Text("b")], vec![]),
            negative: None,
        };
        let joined = PatternSpec {
            grouping: WEST,
            positive: (vec![AffixPart::Text("ab")], vec![]),
            negative: None,
        };
        assert_eq!(
            patterns(&[(Style::Percent, split)]),
            patterns(&[(Style::Percent, joined)])
        );
    }
}
