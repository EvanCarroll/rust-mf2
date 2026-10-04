//! Client view of the `currency.data` LOCALE entry, v1:
//! the configured currencies'
//! symbols, names and fraction digits, and the locale's name patterns —
//! what `:currency` formats with.
//!
//! Client-path code: borrowing, allocation-free, panic-free, fmt-free.
//! [`Currencies::parse`] reads the header and bounds the index (O(1));
//! [`Currencies::get`] binary-searches the index and parses one record.
//! Nothing is walked at load.

use crate::bytes::{Cur, u32_at};
use crate::format::locale_key;
use crate::number::{Forms, Pattern, Template, str8};
use crate::reader::Catalog;

/// Entry flag: narrow symbols are carried.
pub(crate) const F_NARROW: u8 = 0x01;
/// Entry flag: display names (and the name patterns) are carried.
pub(crate) const F_NAMES: u8 = 0x02;

/// Record head: bits 0–3, the fraction digits.
pub(crate) const H_DIGITS: u8 = 0x0f;
/// Record head: a rounding increment (varint) follows.
pub(crate) const H_ROUNDING: u8 = 0x10;
/// Record head: a symbol is stored (else the symbol is the ISO code).
pub(crate) const H_SYMBOL: u8 = 0x20;
/// Record head: a narrow symbol is stored (else it is the symbol).
pub(crate) const H_NARROW: u8 = 0x40;
/// Record head: a display name is stored (else the name is the ISO code).
pub(crate) const H_NAME: u8 = 0x80;

/// Edges byte bit 4: a currency-specific standard pattern follows.
pub(crate) const E_PATTERN: u8 = 0x10;
/// Edges byte bit 5: a currency-specific decimal separator follows.
pub(crate) const E_DECIMAL: u8 = 0x20;
/// Edges byte bit 6: a currency-specific group separator follows.
pub(crate) const E_GROUP: u8 = 0x40;

/// Bytes per index entry: the code and a `u32` offset.
const INDEX_STRIDE: usize = 7;

/// Which ends of a currency symbol are in CLDR's `currencyMatch` set,
/// `[[:^S:]&[:^Z:]]` — neither a symbol nor a separator: a letter, a digit,
/// punctuation. Where the symbol touches the number with such a character,
/// `:currency` uses the pattern's `…-alpha` style and CLDR's currency
/// spacing inserts U+00A0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Edges {
    /// The first scalar: it touches the number when the symbol follows it
    /// (`12CHF`).
    pub first: bool,
    /// The last scalar: it touches the number when the symbol precedes it
    /// (`CHF12`).
    pub last: bool,
}

impl Edges {
    const fn from_bits(b: u8) -> Edges {
        Edges {
            first: b & 1 != 0,
            last: b & 2 != 0,
        }
    }
}

/// A `currency.data` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Currencies<'a> {
    flags: u8,
    default_digits: u8,
    name_patterns: Forms<'a>,
    index: &'a [u8],
    records: &'a [u8],
}

impl<'a> Currencies<'a> {
    /// Parses the header and bounds the index; `None` when malformed. The
    /// records are read by [`Currencies::get`].
    pub fn parse(entry: &'a [u8]) -> Option<Currencies<'a>> {
        let mut c = Cur::new(entry, 0);
        let flags = c.u8()?;
        if flags & !(F_NARROW | F_NAMES) != 0 {
            return None;
        }
        let default_digits = c.u8()?;
        let name_patterns = if flags & F_NAMES != 0 {
            Forms::read(&mut c, true)?
        } else {
            Forms::EMPTY
        };
        let lo = c.u8()?;
        let hi = c.u8()?;
        let n = usize::from(u16::from_le_bytes([lo, hi]));
        let index = c.take(n.checked_mul(INDEX_STRIDE)?)?;
        let records = entry.get(c.pos()..)?;
        Some(Currencies {
            flags,
            default_digits,
            name_patterns,
            index,
            records,
        })
    }

    /// The catalog's `currency.data` entry; `None` when it has none.
    pub fn of(catalog: &'a Catalog) -> Option<Currencies<'a>> {
        Currencies::parse(catalog.locale_entry(locale_key::CURRENCY_DATA)?)
    }

    /// CLDR's default fraction digits (`fractions/DEFAULT`), for a code the
    /// entry does not have.
    pub const fn default_digits(&self) -> u8 {
        self.default_digits
    }

    /// Whether narrow symbols are carried (`currencyDisplay=narrowSymbol`).
    pub const fn narrow_carried(&self) -> bool {
        self.flags & F_NARROW != 0
    }

    /// Whether display names are carried (`currencyDisplay=name`).
    pub const fn names_carried(&self) -> bool {
        self.flags & F_NAMES != 0
    }

    /// The number of currencies.
    pub const fn len(&self) -> usize {
        self.index.len() / INDEX_STRIDE
    }

    /// Whether the entry has no currency.
    pub const fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// The locale's pattern for `currencyDisplay=name` for plural
    /// `category` (0 zero … 5 other; else `other`'s): `{0}` the formatted
    /// number, `{1}` the display name. `None` when names are not carried.
    pub fn name_pattern(&self, category: u8) -> Option<Template<'a>> {
        self.name_patterns.get(category).map(Template::trusted)
    }

    fn code_at(&self, i: usize) -> Option<[u8; 3]> {
        let at = i.checked_mul(INDEX_STRIDE)?;
        match self.index.get(at..at.checked_add(3)?)? {
            &[a, b, c] => Some([a, b, c]),
            _ => None,
        }
    }

    /// The codes, in order.
    pub fn codes(&self) -> impl Iterator<Item = [u8; 3]> + '_ {
        (0..self.len()).filter_map(|i| self.code_at(i))
    }

    /// The currency `code` (upper-case ISO 4217 letters); `None` when the
    /// entry does not have it or its record is malformed. A code outside the
    /// configured set formats with its code as symbol and name and the
    /// default fraction digits (UTS #35's fallback).
    pub fn get(&self, code: [u8; 3]) -> Option<Currency<'a>> {
        let (mut lo, mut hi) = (0usize, self.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let here = self.code_at(mid)?;
            match here.cmp(&code) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => {
                    let start = mid.checked_mul(INDEX_STRIDE)?;
                    let at = start.checked_add(3)?;
                    let off = u32_at(self.index, at)? as usize;
                    let text = core::str::from_utf8(self.index.get(start..at)?).ok()?;
                    return Currency::parse(text, self.records, off, self.flags);
                }
            }
        }
        None
    }

    /// Whether the whole entry is well-formed: codes upper-case and strictly
    /// ascending, offsets strictly ascending, every record parses and ends
    /// where the next begins, nothing trails. Linear; the writer and tests
    /// use it — a client need not.
    pub fn is_valid(&self) -> bool {
        let n = self.len();
        let mut prev: Option<([u8; 3], usize)> = None;
        for i in 0..n {
            let Some(code) = self.code_at(i) else {
                return false;
            };
            let Some(off) = i
                .checked_mul(INDEX_STRIDE)
                .and_then(|a| u32_at(self.index, a + 3))
            else {
                return false;
            };
            let off = off as usize;
            if !code.iter().all(u8::is_ascii_uppercase)
                || prev.is_some_and(|(c, o)| code <= c || off <= o)
            {
                return false;
            }
            if let Some((_, o)) = prev
                && Currency::end(self.records, o, self.flags) != Some(off)
            {
                return false;
            }
            if i == 0 && off != 0 {
                return false;
            }
            prev = Some((code, off));
        }
        match prev {
            Some((_, o)) => Currency::end(self.records, o, self.flags) == Some(self.records.len()),
            None => self.records.is_empty(),
        }
    }
}

/// One currency of a [`Currencies`] entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Currency<'a> {
    code: &'a str,
    head: u8,
    rounding: u32,
    edges: u8,
    symbol: &'a str,
    narrow: Option<&'a str>,
    pattern: Option<Pattern<'a>>,
    decimal: Option<&'a str>,
    group: Option<&'a str>,
    name: Option<&'a str>,
    names: Forms<'a>,
}

impl<'a> Currency<'a> {
    fn read(code: &'a str, c: &mut Cur<'a>, flags: u8) -> Option<Currency<'a>> {
        let head = c.u8()?;
        let rounding = if head & H_ROUNDING != 0 {
            c.varint()?
        } else {
            0
        };
        let edges = c.u8()?;
        if edges & 0x80 != 0 {
            return None;
        }
        let symbol = if head & H_SYMBOL != 0 { str8(c)? } else { code };
        let narrow = match (flags & F_NARROW != 0, head & H_NARROW != 0) {
            (true, true) => Some(str8(c)?),
            (true, false) => Some(symbol),
            (false, false) => None,
            (false, true) => return None,
        };
        let pattern = if edges & E_PATTERN != 0 {
            let len = c.u8()?;
            Some(Pattern::parse(c.take(usize::from(len))?)?)
        } else {
            None
        };
        let decimal = if edges & E_DECIMAL != 0 {
            Some(str8(c)?)
        } else {
            None
        };
        let group = if edges & E_GROUP != 0 {
            Some(str8(c)?)
        } else {
            None
        };
        let (name, names) = match (flags & F_NAMES != 0, head & H_NAME != 0) {
            (true, has) => {
                let name = if has { str8(c)? } else { code };
                (Some(name), Forms::read(c, false)?)
            }
            (false, false) => (None, Forms::EMPTY),
            (false, true) => return None,
        };
        Some(Currency {
            code,
            head,
            rounding,
            edges,
            symbol,
            narrow,
            pattern,
            decimal,
            group,
            name,
            names,
        })
    }

    fn parse(code: &'a str, records: &'a [u8], off: usize, flags: u8) -> Option<Currency<'a>> {
        let mut c = Cur::new(records, off);
        Currency::read(code, &mut c, flags)
    }

    /// Where the record at `off` ends.
    fn end(records: &[u8], off: usize, flags: u8) -> Option<usize> {
        let mut c = Cur::new(records, off);
        Currency::read("XXX", &mut c, flags)?;
        Some(c.pos())
    }

    /// The ISO 4217 code.
    pub const fn code(&self) -> &'a str {
        self.code
    }

    /// The fraction digits (`fractionDigits=auto`), from CLDR's
    /// `currencyData` (`_digits`).
    pub const fn fraction_digits(&self) -> u8 {
        self.head & H_DIGITS
    }

    /// The rounding increment in units of the last fraction digit
    /// (`_rounding`); 0 = none (every currency in CLDR 48.2.1).
    pub const fn rounding_increment(&self) -> u32 {
        self.rounding
    }

    /// The symbol (`currencyDisplay=symbol`); the ISO code when CLDR has
    /// none for this locale.
    pub const fn symbol(&self) -> &'a str {
        self.symbol
    }

    /// The narrow symbol (`currencyDisplay=narrowSymbol`); the symbol when
    /// CLDR has none; `None` when narrow symbols are not carried.
    pub const fn narrow_symbol(&self) -> Option<&'a str> {
        self.narrow
    }

    /// The display name (`displayName`; the ISO code when CLDR has none);
    /// `None` when names are not carried.
    pub const fn name(&self) -> Option<&'a str> {
        self.name
    }

    /// The display name for plural `category` (`displayName-count-*`: the
    /// category's, else `other`'s, else [`Currency::name`]); `None` when
    /// names are not carried.
    pub fn name_for(&self, category: u8) -> Option<&'a str> {
        self.names.get(category).or(self.name)
    }

    /// The currency's own standard pattern, which replaces the locale's
    /// `Currency` / `CurrencyAlpha` styles for it (CLDR `currencies/*/pattern`:
    /// `en-DE` EUR `¤#,##0.00`, `tr` TRY).
    pub const fn pattern(&self) -> Option<Pattern<'a>> {
        self.pattern
    }

    /// The currency's own decimal separator, which replaces the locale's for
    /// it (`pt-CV` CVE: `$`).
    pub const fn decimal(&self) -> Option<&'a str> {
        self.decimal
    }

    /// The currency's own group separator.
    pub const fn group(&self) -> Option<&'a str> {
        self.group
    }

    /// Which ends of [`Currency::symbol`] are letter-like ([`Edges`]).
    pub const fn symbol_edges(&self) -> Edges {
        Edges::from_bits(self.edges)
    }

    /// Which ends of [`Currency::narrow_symbol`] are letter-like.
    pub const fn narrow_edges(&self) -> Edges {
        Edges::from_bits((self.edges >> 2) & 0x03)
    }
}
