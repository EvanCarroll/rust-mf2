//! Client views of the number LOCALE entries: [`Symbols`] over
//! `number.symbols` and [`Patterns`] over
//! `number.patterns`, version 1 each — what `mf2-fn-number` formats with.
//!
//! Client-path code: borrowing, allocation-free, panic-free, fmt-free. A
//! view parses only what it is asked for, when it is asked; malformed bytes
//! give `None` (an entry) or skip nothing and stop (an iterator). Nothing is
//! walked at load: `Catalog::new` treats these entries as opaque, because a
//! bad one can only cost the numbers that read it (F4).

use crate::bytes::Cur;
use crate::format::locale_key;
use crate::reader::Catalog;

/// Affix placeholder byte: the sign position (`-` in a CLDR pattern).
pub(crate) const PH_SIGN: u8 = 0x01;
/// Affix placeholder byte: the percent sign (`%`).
pub(crate) const PH_PERCENT: u8 = 0x02;
/// Affix placeholder byte: the currency symbol (`¤`).
pub(crate) const PH_CURRENCY: u8 = 0x03;

/// Grouping sizes, from the integer part of a CLDR pattern: `primary` is the
/// size of the group next to the decimal separator, `secondary` of every
/// group further left (`#,##,##0` → 3, 2; `#,##0` → 3, 3). A primary size of
/// 0 means the pattern does not group. One byte in the entries: bits 0–3
/// primary, bits 4–7 secondary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Grouping {
    pub primary: u8,
    pub secondary: u8,
}

impl Grouping {
    /// No grouping.
    pub const NONE: Grouping = Grouping {
        primary: 0,
        secondary: 0,
    };

    /// From the entry byte.
    pub const fn from_byte(b: u8) -> Grouping {
        Grouping {
            primary: b & 0x0f,
            secondary: b >> 4,
        }
    }

    /// The entry byte; `None` when a size exceeds 15.
    pub const fn to_byte(self) -> Option<u8> {
        if self.primary > 15 || self.secondary > 15 {
            return None;
        }
        Some(self.secondary << 4 | self.primary)
    }

    /// Whether a group separator follows the integer digit of magnitude `m`
    /// (0 = units; `m ≥ 1`), writing left to right, when grouping applies at
    /// all (`useGrouping`, minimum grouping digits: the formatter decides).
    /// `false` for `m = 0` and when the pattern does not group.
    pub fn separator_after(self, m: u32) -> bool {
        let p = u32::from(self.primary);
        if p == 0 || m < p {
            return false;
        }
        let s = if self.secondary == 0 {
            p
        } else {
            u32::from(self.secondary)
        };
        (m - p).is_multiple_of(s)
    }
}

/// The ten digits of the catalog's numbering system.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Digits<'a> {
    /// Empty for ASCII; else the ten digits, `width` bytes each.
    text: &'a str,
    width: u8,
}

/// The ASCII digits, for [`Digits::digit`].
const ASCII_DIGITS: &str = "0123456789";

impl<'a> Digits<'a> {
    /// ASCII `0`–`9` (the entry has no digits: `latn`).
    pub const ASCII: Digits<'static> = Digits { text: "", width: 0 };

    /// The digits field (the rest of a `number.symbols` entry): empty =
    /// ASCII, else ten digits of one UTF-8 width 1–4, as `10 × width` bytes.
    fn parse(b: &'a [u8]) -> Option<Digits<'a>> {
        if b.is_empty() {
            return Some(Digits::ASCII);
        }
        let n = b.len();
        if !n.is_multiple_of(10) || n > 40 {
            return None;
        }
        let text = core::str::from_utf8(b).ok()?;
        let w = n / 10;
        let mut at = 0;
        while at < n {
            if !text.is_char_boundary(at) {
                return None;
            }
            at += w;
        }
        Some(Digits {
            text,
            width: u8::try_from(w).ok()?,
        })
    }

    /// Whether these are the ASCII digits.
    pub const fn is_ascii(&self) -> bool {
        self.width == 0
    }

    /// Digit `d` (0–9) as text; `""` for `d > 9`.
    pub fn digit(&self, d: u8) -> &'a str {
        if d > 9 {
            return "";
        }
        let (text, w) = if self.width == 0 {
            (ASCII_DIGITS, 1)
        } else {
            (self.text, usize::from(self.width))
        };
        let at = usize::from(d) * w;
        text.get(at..at + w).unwrap_or("")
    }

    /// The ten digits in order (`"0123456789"` for ASCII).
    pub fn as_str(&self) -> &'a str {
        if self.width == 0 {
            ASCII_DIGITS
        } else {
            self.text
        }
    }
}

/// Reads `u8 len · UTF-8{len}`.
pub(crate) fn str8<'a>(c: &mut Cur<'a>) -> Option<&'a str> {
    let n = c.u8()?;
    core::str::from_utf8(c.take(usize::from(n))?).ok()
}

/// A `number.symbols` entry, v1 (§4.2): the symbols, grouping and digits of
/// the catalog's one numbering system (its locale's default, or the tag's
/// `-u-nu-`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Symbols<'a> {
    grouping: Grouping,
    min_grouping: u8,
    decimal: &'a str,
    group: &'a str,
    minus: &'a str,
    plus: &'a str,
    percent: &'a str,
    digits: Digits<'a>,
}

impl<'a> Symbols<'a> {
    /// Parses a `number.symbols` payload; `None` when it is malformed
    /// (truncated, a string not UTF-8, digits of a bad size or width).
    pub fn parse(entry: &'a [u8]) -> Option<Symbols<'a>> {
        let mut c = Cur::new(entry, 0);
        let grouping = Grouping::from_byte(c.u8()?);
        let min_grouping = c.u8()?;
        let decimal = str8(&mut c)?;
        let group = str8(&mut c)?;
        let minus = str8(&mut c)?;
        let plus = str8(&mut c)?;
        let percent = str8(&mut c)?;
        let digits = Digits::parse(entry.get(c.pos()..)?)?;
        Some(Symbols {
            grouping,
            min_grouping,
            decimal,
            group,
            minus,
            plus,
            percent,
            digits,
        })
    }

    /// The catalog's `number.symbols` entry, parsed; `None` when the catalog
    /// has none (the corpus formats no number, or `fn-number` is off) or it is
    /// malformed.
    pub fn of(catalog: &'a Catalog) -> Option<Symbols<'a>> {
        Symbols::parse(catalog.locale_entry(locale_key::NUMBER_SYMBOLS)?)
    }

    /// The grouping of the locale's decimal pattern (plain `:number`,
    /// `:integer`, `:offset` and unannotated numbers).
    pub const fn grouping(&self) -> Grouping {
        self.grouping
    }

    /// CLDR's `minimumGroupingDigits` (`useGrouping=auto`): group only when
    /// the integer part has at least `primary + this` digits.
    pub const fn minimum_grouping_digits(&self) -> u8 {
        self.min_grouping
    }

    /// The decimal separator.
    pub const fn decimal(&self) -> &'a str {
        self.decimal
    }

    /// The group separator.
    pub const fn group(&self) -> &'a str {
        self.group
    }

    /// The minus sign (may carry bidi marks, e.g. `ar`'s U+200E / U+061C).
    pub const fn minus(&self) -> &'a str {
        self.minus
    }

    /// The plus sign.
    pub const fn plus(&self) -> &'a str {
        self.plus
    }

    /// The percent sign.
    pub const fn percent(&self) -> &'a str {
        self.percent
    }

    /// The digits (ASCII unless the numbering system is not `latn`).
    pub const fn digits(&self) -> Digits<'a> {
        self.digits
    }
}

/// The styles of a `number.patterns` entry, v1 (§4.3). The record byte is
/// the discriminant; records are in ascending style order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Style {
    /// `percentFormats/standard` (`:percent`).
    Percent = 1,
    /// `currencyFormats/standard`.
    Currency = 2,
    /// `currencyFormats/standard-alphaNextToNumber`: used instead of
    /// `Currency` when the currency symbol's letter would touch the digits.
    /// Absent when CLDR has none or it equals `Currency` — then `Currency`.
    CurrencyAlpha = 3,
    /// `currencyFormats/standard-noCurrency` (`currencyDisplay=never`).
    CurrencyNoSymbol = 4,
    /// `currencyFormats/accounting` (`currencySign=accounting`).
    Accounting = 5,
    /// `currencyFormats/accounting-alphaNextToNumber`; absent = `Accounting`.
    AccountingAlpha = 6,
    /// `currencyFormats/accounting-noCurrency`.
    AccountingNoSymbol = 7,
}

impl Style {
    /// Every style, in record order.
    pub const ALL: [Style; 7] = [
        Style::Percent,
        Style::Currency,
        Style::CurrencyAlpha,
        Style::CurrencyNoSymbol,
        Style::Accounting,
        Style::AccountingAlpha,
        Style::AccountingNoSymbol,
    ];

    /// From the record byte.
    pub const fn from_u8(b: u8) -> Option<Style> {
        Some(match b {
            1 => Style::Percent,
            2 => Style::Currency,
            3 => Style::CurrencyAlpha,
            4 => Style::CurrencyNoSymbol,
            5 => Style::Accounting,
            6 => Style::AccountingAlpha,
            7 => Style::AccountingNoSymbol,
            _ => return None,
        })
    }

    /// The style an absent record of this style falls back to: the base
    /// pattern for the `…Alpha` variants, `None` for the others.
    pub const fn base(self) -> Option<Style> {
        match self {
            Style::CurrencyAlpha => Some(Style::Currency),
            Style::AccountingAlpha => Some(Style::Accounting),
            _ => None,
        }
    }
}

/// A `number.patterns` entry, v1 (§4.3): one record per style the corpus
/// uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Patterns<'a> {
    entry: &'a [u8],
}

impl<'a> Patterns<'a> {
    /// A view of a `number.patterns` payload (nothing is read yet).
    pub const fn new(entry: &'a [u8]) -> Patterns<'a> {
        Patterns { entry }
    }

    /// The catalog's `number.patterns` entry; `None` when it has none.
    pub fn of(catalog: &'a Catalog) -> Option<Patterns<'a>> {
        catalog
            .locale_entry(locale_key::NUMBER_PATTERNS)
            .map(Patterns::new)
    }

    /// The record of `style`, parsed; `None` when the entry has none or the
    /// entry is malformed up to it. No fallback: see [`Patterns::resolve`].
    pub fn get(&self, style: Style) -> Option<Pattern<'a>> {
        let want = style as u8;
        let mut c = Cur::new(self.entry, 0);
        let mut prev = 0u8;
        while !c.at_end() {
            let s = c.u8()?;
            let len = c.u8()?;
            let body = c.take(usize::from(len))?;
            if s <= prev {
                return None;
            }
            prev = s;
            if s == want {
                return Pattern::parse(body);
            }
            if s > want {
                return None;
            }
        }
        None
    }

    /// The record of `style`, or of its [`Style::base`] when the entry has
    /// none of its own (the canonical writer omits an `…Alpha` record that
    /// CLDR lacks or that equals its base).
    pub fn resolve(&self, style: Style) -> Option<Pattern<'a>> {
        self.get(style)
            .or_else(|| style.base().and_then(|b| self.get(b)))
    }

    /// Whether the whole entry is well-formed: known styles, strictly
    /// ascending, every record parses and nothing trails. Linear; the writer
    /// and tests use it — a client need not.
    pub fn is_valid(&self) -> bool {
        let mut c = Cur::new(self.entry, 0);
        let mut prev = 0u8;
        while !c.at_end() {
            let (Some(s), Some(len)) = (c.u8(), c.u8()) else {
                return false;
            };
            let Some(body) = c.take(usize::from(len)) else {
                return false;
            };
            if s <= prev || Style::from_u8(s).is_none() || Pattern::parse(body).is_none() {
                return false;
            }
            prev = s;
        }
        true
    }

    /// The known styles present, in order (unknown ones skipped; stops at
    /// the first truncated record).
    pub fn styles(&self) -> impl Iterator<Item = Style> + 'a {
        let mut c = Cur::new(self.entry, 0);
        core::iter::from_fn(move || {
            loop {
                let s = c.u8()?;
                let len = c.u8()?;
                c.skip(usize::from(len))?;
                if let Some(style) = Style::from_u8(s) {
                    return Some(style);
                }
            }
        })
    }
}

/// One pattern of a `number.patterns` record: its grouping and its affixes.
/// The number part of a CLDR pattern carries nothing else MF2 uses (fraction
/// digits come from the function's options and, for `:currency`, the
/// currency).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pattern<'a> {
    grouping: Grouping,
    positive: Affixes<'a>,
    negative: Option<Affixes<'a>>,
}

/// The sign a formatted number shows, after `signDisplay` has decided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignShown {
    /// No sign.
    None,
    /// The minus sign (a negative number, or `-0` with `signDisplay` showing it).
    Minus,
    /// The plus sign (`signDisplay=always` / `exceptZero`).
    Plus,
}

/// The affixes to write for one number, from [`Pattern::signed`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Signed<'a> {
    /// Written before the digits.
    pub prefix: Affix<'a>,
    /// Written after the digits.
    pub suffix: Affix<'a>,
    /// Write the sign symbol first, before `prefix`: the chosen affixes have
    /// no sign position of their own. When `false`, the sign (if any) is
    /// written where an [`AffixPart::Sign`] appears — possibly nowhere, as in
    /// accounting's parentheses.
    pub sign_first: bool,
}

impl<'a> Pattern<'a> {
    /// A record body: `u8 grouping · affix · affix · [affix · affix]`.
    pub(crate) fn parse(body: &'a [u8]) -> Option<Pattern<'a>> {
        let mut c = Cur::new(body, 0);
        let grouping = Grouping::from_byte(c.u8()?);
        let positive = Affixes {
            prefix: Affix::read(&mut c)?,
            suffix: Affix::read(&mut c)?,
        };
        let negative = if c.at_end() {
            None
        } else {
            Some(Affixes {
                prefix: Affix::read(&mut c)?,
                suffix: Affix::read(&mut c)?,
            })
        };
        c.at_end().then_some(Pattern {
            grouping,
            positive,
            negative,
        })
    }

    /// The grouping of this pattern (it may differ from the decimal
    /// pattern's: `as` accounting groups by 3 where its decimals group 3, 2).
    pub const fn grouping(&self) -> Grouping {
        self.grouping
    }

    /// The positive subpattern's affixes.
    pub const fn positive(&self) -> Affixes<'a> {
        self.positive
    }

    /// The explicit negative subpattern's affixes, if the pattern has one.
    pub const fn negative(&self) -> Option<Affixes<'a>> {
        self.negative
    }

    /// The affixes for a number showing `sign` (UTS #35 §3.2 as ICU applies
    /// it): no sign → the positive affixes; minus → the negative affixes if
    /// the pattern has them (their sign position renders the minus sign),
    /// else the positive ones with the sign first; plus → the negative
    /// affixes if they have a sign position (rendering the plus sign), else
    /// the positive ones with the sign first.
    pub fn signed(&self, sign: SignShown) -> Signed<'a> {
        let pos = Signed {
            prefix: self.positive.prefix,
            suffix: self.positive.suffix,
            sign_first: !matches!(sign, SignShown::None),
        };
        let Some(neg) = self.negative else {
            return pos;
        };
        let neg = Signed {
            prefix: neg.prefix,
            suffix: neg.suffix,
            sign_first: false,
        };
        match sign {
            SignShown::Minus => neg,
            SignShown::Plus if neg.prefix.has_sign() || neg.suffix.has_sign() => neg,
            SignShown::None | SignShown::Plus => pos,
        }
    }
}

/// A prefix and a suffix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Affixes<'a> {
    pub prefix: Affix<'a>,
    pub suffix: Affix<'a>,
}

/// One affix: literal text with placeholders. Stored as UTF-8 in which the
/// bytes 0x01 (sign), 0x02 (percent sign) and 0x03 (currency symbol) stand
/// for the placeholders; no other byte below 0x20 occurs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Affix<'a>(&'a str);

/// One piece of an [`Affix`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AffixPart<'a> {
    /// Literal text (bidi marks and spaces included: U+200F, U+00A0, …).
    Text(&'a str),
    /// The sign position (`-`): the minus or the plus sign, per
    /// [`Pattern::signed`].
    Sign,
    /// The percent sign ([`Symbols::percent`]).
    Percent,
    /// The currency symbol, in the form `currencyDisplay` asks for.
    Currency,
}

impl<'a> Affix<'a> {
    /// The empty affix.
    pub const EMPTY: Affix<'static> = Affix("");

    fn read(c: &mut Cur<'a>) -> Option<Affix<'a>> {
        let text = str8(c)?;
        if text
            .bytes()
            .any(|b| b < 0x20 && !matches!(b, PH_SIGN | PH_PERCENT | PH_CURRENCY))
        {
            return None;
        }
        Some(Affix(text))
    }

    /// Whether the affix is empty.
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The encoded form (text with placeholder bytes), for tests and dumps.
    pub const fn encoded(&self) -> &'a str {
        self.0
    }

    /// Whether the affix has a sign position.
    pub fn has_sign(&self) -> bool {
        self.0.bytes().any(|b| b == PH_SIGN)
    }

    /// Whether the affix has a currency symbol.
    pub fn has_currency(&self) -> bool {
        self.0.bytes().any(|b| b == PH_CURRENCY)
    }

    /// The pieces, in order.
    pub fn parts(&self) -> AffixParts<'a> {
        AffixParts { rest: self.0 }
    }
}

/// The pieces of an [`Affix`], from [`Affix::parts`].
#[derive(Clone, Debug)]
pub struct AffixParts<'a> {
    rest: &'a str,
}

impl<'a> Iterator for AffixParts<'a> {
    type Item = AffixPart<'a>;

    fn next(&mut self) -> Option<AffixPart<'a>> {
        let b = self.rest.as_bytes();
        let first = *b.first()?;
        let placeholder = match first {
            PH_SIGN => Some(AffixPart::Sign),
            PH_PERCENT => Some(AffixPart::Percent),
            PH_CURRENCY => Some(AffixPart::Currency),
            _ => None,
        };
        if let Some(p) = placeholder {
            self.rest = self.rest.get(1..).unwrap_or("");
            return Some(p);
        }
        // Text up to the next placeholder byte (ASCII, so a char boundary).
        let mut end = 1;
        while let Some(&x) = b.get(end) {
            if x < 0x20 {
                break;
            }
            end += 1;
        }
        let text = self.rest.get(..end).unwrap_or("");
        self.rest = self.rest.get(end..).unwrap_or("");
        Some(AffixPart::Text(text))
    }
}

/// Template placeholder byte: `{0}` (the formatted number).
pub(crate) const T_ARG0: u8 = 0x01;
/// Template placeholder byte: `{1}` (a name: the currency's, a unit's).
pub(crate) const T_ARG1: u8 = 0x02;

/// A CLDR message-like pattern (`{0} km`, `{0} {1}`, `{0}/{1}`), stored as
/// UTF-8 in which the byte 0x01 stands for `{0}` and 0x02 for `{1}`; no other
/// byte below 0x20 occurs. A template may
/// have no placeholder at all: CLDR writes some `one`/`two` unit forms with
/// the number in the word (`ar` `دورتان`, two revolutions).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Template<'a>(&'a str);

/// One piece of a [`Template`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TemplatePart<'a> {
    /// Literal text.
    Text(&'a str),
    /// `{0}`: the formatted number (or, in `per`, the formatted numerator).
    Arg0,
    /// `{1}`: the name (the currency's display name; in `per`, the
    /// denominator unit).
    Arg1,
}

impl<'a> Template<'a> {
    /// Reads `u8 len · UTF-8{len}` and checks the bytes below 0x20.
    pub(crate) fn read(c: &mut Cur<'a>) -> Option<Template<'a>> {
        let text = str8(c)?;
        if text.bytes().any(|b| b < 0x20 && b != T_ARG0 && b != T_ARG1) {
            return None;
        }
        Some(Template(text))
    }

    /// A string [`Template::read`] (or [`Forms::read`]) has checked.
    pub(crate) const fn trusted(s: &'a str) -> Template<'a> {
        Template(s)
    }

    /// The encoded form (text with placeholder bytes), for tests and dumps.
    pub const fn encoded(&self) -> &'a str {
        self.0
    }

    /// The pieces, in order.
    pub fn parts(&self) -> TemplateParts<'a> {
        TemplateParts { rest: self.0 }
    }
}

/// The pieces of a [`Template`], from [`Template::parts`].
#[derive(Clone, Debug)]
pub struct TemplateParts<'a> {
    rest: &'a str,
}

impl<'a> Iterator for TemplateParts<'a> {
    type Item = TemplatePart<'a>;

    fn next(&mut self) -> Option<TemplatePart<'a>> {
        let b = self.rest.as_bytes();
        let first = *b.first()?;
        let arg = match first {
            T_ARG0 => Some(TemplatePart::Arg0),
            T_ARG1 => Some(TemplatePart::Arg1),
            _ => None,
        };
        if let Some(a) = arg {
            self.rest = self.rest.get(1..).unwrap_or("");
            return Some(a);
        }
        let mut end = 1;
        while let Some(&x) = b.get(end) {
            if x < 0x20 {
                break;
            }
            end += 1;
        }
        let text = self.rest.get(..end).unwrap_or("");
        self.rest = self.rest.get(end..).unwrap_or("");
        Some(TemplatePart::Text(text))
    }
}

/// The plural category code of a form: 0 zero, 1 one, 2 two, 3 few, 4
/// many, 5 other — `mf2_runtime::Category as u8`.
pub const OTHER: u8 = 5;

/// A list of plural forms, `u8 k · (u8 category · str8){k}`, categories
/// strictly ascending (0–5). Canonically a form equal to `other` is omitted,
/// so a lookup falls back to `other` (CLDR's rule).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Forms<'a> {
    body: &'a [u8],
    k: u8,
}

impl<'a> Forms<'a> {
    pub(crate) const EMPTY: Forms<'static> = Forms { body: &[], k: 0 };

    /// Reads the list and checks it (categories, order, strings); `templates`
    /// also checks each string's placeholder bytes. Linear in its length.
    pub(crate) fn read(c: &mut Cur<'a>, templates: bool) -> Option<Forms<'a>> {
        let k = c.u8()?;
        let start = c.pos();
        let mut prev: Option<u8> = None;
        for _ in 0..k {
            let cat = c.u8()?;
            if cat > OTHER || prev.is_some_and(|p| cat <= p) {
                return None;
            }
            prev = Some(cat);
            if templates {
                Template::read(c)?;
            } else {
                let s = str8(c)?;
                if s.bytes().any(|b| b < 0x20) {
                    return None;
                }
            }
        }
        let body = c.bytes().get(start..c.pos())?;
        Some(Forms { body, k })
    }

    /// The form of `category`, else `other`'s; `None` when neither is there.
    pub(crate) fn get(&self, category: u8) -> Option<&'a str> {
        let mut c = Cur::new(self.body, 0);
        let mut other = None;
        for _ in 0..self.k {
            let cat = c.u8()?;
            let s = str8(&mut c)?;
            if cat == category {
                return Some(s);
            }
            if cat == OTHER {
                other = Some(s);
            }
        }
        other
    }

    /// Whether there is no form.
    pub(crate) const fn is_empty(&self) -> bool {
        self.k == 0
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::{AffixPart, Grouping, Patterns, SignShown, Style, Symbols};

    #[test]
    fn grouping_separators() {
        let western = Grouping {
            primary: 3,
            secondary: 3,
        };
        let indian = Grouping {
            primary: 3,
            secondary: 2,
        };
        let at = |g: Grouping| {
            (0..10)
                .filter(|&m| g.separator_after(m))
                .collect::<alloc::vec::Vec<_>>()
        };
        assert_eq!(at(western), [3, 6, 9]);
        assert_eq!(at(indian), [3, 5, 7, 9]);
        assert!(at(Grouping::NONE).is_empty());
        assert_eq!(Grouping::from_byte(0x23), indian);
        assert_eq!(indian.to_byte(), Some(0x23));
    }

    #[test]
    fn symbols_malformed() {
        // en: grouping 3/3, min 1, "." "," "-" "+" "%".
        let en = [0x33, 1, 1, b'.', 1, b',', 1, b'-', 1, b'+', 1, b'%'];
        let s = Symbols::parse(&en).expect("en");
        assert_eq!(s.decimal(), ".");
        assert!(s.digits().is_ascii());
        assert_eq!(s.digits().digit(7), "7");
        assert_eq!(s.digits().digit(10), "");
        for len in 0..en.len() {
            assert!(
                Symbols::parse(en.get(..len).unwrap_or(&[])).is_none(),
                "{len}"
            );
        }
        let mut bad_digits = alloc::vec::Vec::from(en);
        bad_digits.extend_from_slice(b"012345678"); // 9 bytes
        assert!(Symbols::parse(&bad_digits).is_none());
        let mut not_utf8 = alloc::vec::Vec::from(en);
        not_utf8[3] = 0xff;
        assert!(Symbols::parse(&not_utf8).is_none());
    }

    #[test]
    fn patterns_lookup_and_sign_rules() {
        // Percent `#,##0%`; accounting `¤#,##0.00;(¤#,##0.00)`.
        let entry = [
            1, 4, 0x33, 0, 1, 0x02, // percent: prefix "", suffix %
            5, 9, 0x33, 1, 0x03, 0, 2, b'(', 0x03, 1, b')', // accounting
        ];
        let p = Patterns::new(&entry);
        assert!(p.is_valid());
        assert_eq!(
            p.styles().collect::<alloc::vec::Vec<_>>(),
            [Style::Percent, Style::Accounting]
        );
        let pct = p.get(Style::Percent).expect("percent");
        let s = pct.signed(SignShown::Minus);
        assert!(s.sign_first && s.prefix.is_empty());
        assert_eq!(
            s.suffix.parts().collect::<alloc::vec::Vec<_>>(),
            [AffixPart::Percent]
        );
        assert!(p.get(Style::Currency).is_none());
        assert!(p.resolve(Style::AccountingAlpha).is_some());
        let acc = p.get(Style::Accounting).expect("accounting");
        let neg = acc.signed(SignShown::Minus);
        assert!(!neg.sign_first);
        assert_eq!(
            neg.prefix.parts().collect::<alloc::vec::Vec<_>>(),
            [AffixPart::Text("("), AffixPart::Currency]
        );
        // Plus: the negative subpattern has no sign position → positive, sign first.
        let plus = acc.signed(SignShown::Plus);
        assert!(plus.sign_first);
        assert!(plus.prefix.has_currency());
        // Truncations (except at the record boundary) and disorder are rejected.
        for len in (1..entry.len()).filter(|&l| l != 6) {
            assert!(
                !Patterns::new(entry.get(..len).unwrap_or(&[])).is_valid(),
                "{len}"
            );
        }
        let swapped = [5, 4, 0x33, 0, 1, 0x02, 1, 4, 0x33, 0, 1, 0x02];
        assert!(!Patterns::new(&swapped).is_valid());
        assert!(Patterns::new(&swapped).get(Style::Percent).is_none());
        // A control byte other than a placeholder.
        let bad = [1, 4, 0x33, 0, 1, 0x04];
        assert!(!Patterns::new(&bad).is_valid());
    }

    #[test]
    fn affix_parts_split_text() {
        // `‏-` + currency + NBSP: "\u{200f}" SIGN CURRENCY "\u{a0}".
        let affix = "\u{200f}\u{1}\u{3}\u{a0}";
        let entry = {
            let mut e = alloc::vec![2u8, 0, 0x33];
            e.push(0);
            e.push(u8::try_from(affix.len()).unwrap_or(0));
            e.extend_from_slice(affix.as_bytes());
            let len = u8::try_from(e.len() - 2).unwrap_or(0);
            e[1] = len;
            e
        };
        let p = Patterns::new(&entry);
        assert!(p.is_valid());
        let pat = p.get(Style::Currency).expect("currency");
        assert_eq!(
            pat.positive()
                .suffix
                .parts()
                .collect::<alloc::vec::Vec<_>>(),
            [
                AffixPart::Text("\u{200f}"),
                AffixPart::Sign,
                AffixPart::Currency,
                AffixPart::Text("\u{a0}")
            ]
        );
    }
}
