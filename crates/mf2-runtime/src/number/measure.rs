//! A number with a currency or a unit (`plans/03-runtime.md` §2.7): what
//! `:currency` and `:unit` (`mf2-fn-number`) resolve to. The runtime only
//! carries it — as a numeric operand it is its number.

use super::Number;

/// A number with a currency or a unit, and the options its function added.
#[derive(Clone)]
#[non_exhaustive]
pub struct Measure<'a> {
    /// The number, with its resolved numeric options.
    pub number: Number,
    /// The currency or the unit.
    pub unit: MeasureUnit<'a>,
    /// The resolving crate's own encoding of the options it adds
    /// (`currencyDisplay`, `currencySign`, `fractionDigits`, `unitDisplay`),
    /// inherited by a later `:currency` / `:unit` that takes this value as
    /// its operand. The runtime never reads it.
    pub flags: u32,
}

impl<'a> Measure<'a> {
    /// A measure of `number` in `unit`.
    pub fn new(number: Number, unit: MeasureUnit<'a>, flags: u32) -> Self {
        Measure {
            number,
            unit,
            flags,
        }
    }
}

/// What a [`Measure`] measures.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MeasureUnit<'a> {
    /// A currency: a well-formed code (`currency_code = 3ALPHA`), upper-cased.
    Currency([u8; 3]),
    /// A unit: a well-formed Unicode unit identifier, as written.
    Unit(&'a str),
}

impl MeasureUnit<'_> {
    /// The currency code or the unit identifier, as text.
    pub fn as_str(&self) -> &str {
        match self {
            MeasureUnit::Currency(c) => core::str::from_utf8(c).unwrap_or(""),
            MeasureUnit::Unit(u) => u,
        }
    }
}
