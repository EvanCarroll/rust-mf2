//! The backends of a domain: who formats its values on a side.
//!
//! A domain is a group of functions — the date functions — and each side
//! that formats it names one backend, with a feature of one of its families.
//! Everything the build and the tools say about backends is derived from an
//! implementation of [`Backend`], so a domain's list is written once.

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
