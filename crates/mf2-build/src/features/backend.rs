//! The backends of a domain: who formats its values on a side.
//!
//! A domain is a group of functions — the number functions, the date
//! functions — and each side that formats it names one backend, with a
//! feature of one of its families. Everything the build and the tools say
//! about backends is derived from an implementation of [`Backend`], so a
//! domain's list is written once.

use super::family::Side;

/// A backend of one domain. The order (`Ord`) is strength, the weakest
/// first: a build with more than one of its side's on formats with the
/// strongest.
pub trait Backend: Copy + Ord + core::fmt::Debug + 'static {
    /// The domain: the feature every backend of it turns on, and the middle
    /// of a backend's feature name (`datetime`).
    const DOMAIN: &'static str;

    /// What a backend of this domain is called in a message: `date
    /// formatter`.
    const NOUN: &'static str;

    /// What this domain's functions format, in a message: `a date`.
    const THING: &'static str;

    /// The domain as an adjective, in a message: `date`.
    const ADJECTIVE: &'static str;

    /// What this domain's formatters format, in a message: `dates`.
    const THINGS: &'static str;

    /// The domain's functions, as a message lists them: `:datetime, :date
    /// or :time`.
    const FUNCTIONS: &'static str;

    /// The backends `side`'s families offer, the strongest first.
    fn offered(side: Side) -> &'static [Self];

    /// The backend the tools recommend for `side`.
    fn recommended(side: Side) -> Self;

    /// The name a family's feature ends with.
    fn name(self) -> &'static str;

    /// What it is, as a message names it.
    fn what(self) -> &'static str;

    /// What it costs on `side`, as a message says it.
    fn cost(self, side: Side) -> &'static str;

    /// A weaker backend whose feature this one's feature turns on with it:
    /// the two are then one backend, this one.
    fn implies(self) -> Option<Self> {
        None
    }
}

/// A date formatter, the weakest first: the order is the rule a build with
/// more than one of its side's on applies, where the strongest formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateBackend {
    /// The ISO stand-in: no locale data, no ICU4X.
    Iso,
    /// The browser's `Intl.DateTimeFormat`: no date data downloaded.
    Intl,
    /// ICU4X over the catalog's `icu.blob`, which the build cuts.
    Icu,
}

impl DateBackend {
    /// Whether it formats from the catalog's date slice (`icu.blob`), which
    /// the build then cuts.
    #[doc(hidden)]
    pub fn reads_slice(self) -> bool {
        matches!(self, DateBackend::Icu)
    }
}

impl Backend for DateBackend {
    const DOMAIN: &'static str = "datetime";
    const NOUN: &'static str = "date formatter";
    const THING: &'static str = "a date";
    const ADJECTIVE: &'static str = "date";
    const THINGS: &'static str = "dates";
    const FUNCTIONS: &'static str = ":datetime, :date or :time";

    fn offered(side: Side) -> &'static [DateBackend] {
        match side {
            Side::Browser => &[DateBackend::Icu, DateBackend::Intl, DateBackend::Iso],
            Side::Native => &[DateBackend::Icu, DateBackend::Iso],
        }
    }

    /// `Intl` in a browser, ICU4X in native code.
    fn recommended(side: Side) -> DateBackend {
        match side {
            Side::Browser => DateBackend::Intl,
            Side::Native => DateBackend::Icu,
        }
    }

    fn name(self) -> &'static str {
        match self {
            DateBackend::Iso => "iso",
            DateBackend::Intl => "intl",
            DateBackend::Icu => "icu",
        }
    }

    fn what(self) -> &'static str {
        match self {
            DateBackend::Iso => "the ISO stand-in",
            DateBackend::Intl => "the browser's Intl.DateTimeFormat",
            DateBackend::Icu => "ICU4X",
        }
    }

    fn cost(self, side: Side) -> &'static str {
        match (self, side) {
            (DateBackend::Iso, _) => "no locale data and no ICU4X",
            (DateBackend::Intl, _) => "+239 B gzipped, and no date data downloaded",
            (DateBackend::Icu, Side::Browser) => {
                "+43 to +100 KB gzipped, and the date slice in each catalog"
            }
            (DateBackend::Icu, Side::Native) => "+298 KB, and the date slices",
        }
    }
}

/// A number formatter, the weakest first: the order is the rule a build with
/// more than one of its side's on applies, where the strongest formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumberBackend {
    /// Plain digits (`1234.5`): no symbols of the language, and no
    /// `:percent`, `:currency` or `:unit`. Plural selection by the catalog's
    /// rules.
    Plain,
    /// The browser's `Intl.NumberFormat` and `Intl.PluralRules`: no number
    /// or plural data downloaded.
    Intl,
    /// `mf2`'s own code over the CLDR data the build cuts into the catalog.
    Builtin,
}

impl NumberBackend {
    /// Whether it formats in the language's own form: its symbols, grouping
    /// and digits, and so `:percent`, `:currency` and `:unit`.
    #[doc(hidden)]
    pub fn localizes(self) -> bool {
        !matches!(self, NumberBackend::Plain)
    }

    /// Whether it selects plurals from the catalog's rules
    /// (`plural.cardinal`, `plural.ordinal`).
    #[doc(hidden)]
    pub fn reads_plural_rules(self) -> bool {
        matches!(self, NumberBackend::Plain | NumberBackend::Builtin)
    }

    /// Whether it formats from the catalog's number data (`number.symbols`,
    /// `number.patterns`, `currency.data`, `unit.data`).
    #[doc(hidden)]
    pub fn reads_number_data(self) -> bool {
        matches!(self, NumberBackend::Builtin)
    }
}

impl Backend for NumberBackend {
    const DOMAIN: &'static str = "number";
    const NOUN: &'static str = "number formatter";
    const THING: &'static str = "a number";
    const ADJECTIVE: &'static str = "number";
    const THINGS: &'static str = "numbers";
    const FUNCTIONS: &'static str = ":number, :integer, :offset, :percent, :currency or :unit";

    fn offered(side: Side) -> &'static [NumberBackend] {
        match side {
            Side::Browser => &[
                NumberBackend::Builtin,
                NumberBackend::Intl,
                NumberBackend::Plain,
            ],
            Side::Native => &[NumberBackend::Builtin, NumberBackend::Plain],
        }
    }

    /// `Intl` in a browser, `mf2`'s own code natively.
    fn recommended(side: Side) -> NumberBackend {
        match side {
            Side::Browser => NumberBackend::Intl,
            Side::Native => NumberBackend::Builtin,
        }
    }

    fn name(self) -> &'static str {
        match self {
            NumberBackend::Plain => "plain",
            NumberBackend::Intl => "intl",
            NumberBackend::Builtin => "builtin",
        }
    }

    fn what(self) -> &'static str {
        match self {
            NumberBackend::Plain => "plain digits",
            NumberBackend::Intl => "the browser's Intl.NumberFormat and Intl.PluralRules",
            NumberBackend::Builtin => "mf2's own code over the catalog's CLDR data",
        }
    }

    fn cost(self, side: Side) -> &'static str {
        match (self, side) {
            (NumberBackend::Plain, _) => {
                "no number data, and neither :percent, :currency nor :unit"
            }
            (NumberBackend::Intl, _) => {
                "545 B gzipped less than `plain`, and no number or plural data downloaded"
            }
            (NumberBackend::Builtin, Side::Browser) => {
                "+2,514 B gzipped over `plain`, and the number data in each catalog"
            }
            (NumberBackend::Builtin, Side::Native) => "+9,920 B over `plain`, and the number data",
        }
    }
}
