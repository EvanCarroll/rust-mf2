//! `mf2-locale-data` — the locale data Rust MF2 catalogs carry, build side
//! only: never linked into a client.
//!
//! The crate ships its data, derived from Unicode CLDR, in `data/`: a build
//! downloads nothing. From CLDR's JSON:
//!
//! * **plural** rules and each locale's **text direction**: the UTS #35 rule parser, the canonical encoder
//!   of the `plural.cardinal` / `plural.ordinal` entries; the CLDR
//!   `@integer`/`@decimal` samples of every locale are tests of the client
//!   evaluator;
//! * **numbers**, from every locale's `numbers.json`: symbols, grouping, numbering systems and
//!   their digits, the percent and currency patterns, deduplicated against
//!   CLDR's parent locales (`data/numbers.txt`), and the `number.symbols` /
//!   `number.patterns` entries built from them (`number`);
//! * **currencies and units**, from every locale's `currencies.json` and
//!   `units.json`
//!   (`data/currencies.txt`, `data/units.txt`, deduplicated against the
//!   parents and CLDR's fallbacks), and the `currency.data` / `unit.data`
//!   entries for the configured sets (`currency`, `unit`).
//!
//! A corpus's needs — which entries, which currencies and units — are
//! `LocaleNeeds` / `NumberNeeds` (`NumberNeeds::add_message` reads them
//! off the data model); `locale_entries` writes the entries.
//!
//! # What 1.x promises here
//!
//! `mf2-build` and `mf2`'s `compile` feature use this crate; an application
//! never names it. Its tables and entry builders are the catalog's layout,
//! which 1.x does not promise (`docs/versioning.md`), so they are hidden
//! from the documentation. What is promised: the errors a build can return
//! from here ([`Error`], [`ParseError`]), the CLDR release the data comes
//! from ([`CLDR_VERSION`]) and a locale's text [`direction`].
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 1.x promises.
//! An application reaches this crate through `mf2-build`
//! and [`mf2`](https://docs.rs/mf2)'s `compile` feature.

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc(hidden)]
pub mod blocks;
#[doc(hidden)]
pub mod currency;
mod direction;
mod error;
#[cfg(feature = "extract")]
#[doc(hidden)]
pub mod extract;
#[cfg(feature = "icu-blob")]
#[doc(hidden)]
pub mod icu_blob;
#[doc(hidden)]
pub mod number;
#[doc(hidden)]
pub mod plural;
#[doc(hidden)]
pub mod template;
#[doc(hidden)]
pub mod unit;

#[doc(hidden)]
pub use currency::CurrencyData;
pub use direction::direction;
pub use error::{Error, ParseError};
pub use mf2_catalog::CldrVersion;
#[doc(hidden)]
pub use number::{
    CurrencyNeeds, NumberData, NumberNeeds, Selection, UnitNeeds, number_data,
    number_locale_entries, number_locales,
};
#[doc(hidden)]
pub use plural::{
    LocaleRules, PluralKind, plural_entry, plural_locale_entries, plural_locales, plural_rules,
};
#[doc(hidden)]
pub use unit::{UnitData, composition, unit_ids};

/// The CLDR release the shipped data comes from (`third_party/cldr-json/PIN`).
pub const CLDR_VERSION: CldrVersion = CldrVersion {
    major: 48,
    minor: 2,
    patch: 1,
};

/// Everything a corpus needs of its locale's data: plural rules of each
/// kind it selects on, and number data ([`NumberNeeds`]). The slicing rule
/// is `plans/02-catalog-format.md` §4.4. Build one with `default()` and set
/// fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[doc(hidden)]
pub struct LocaleNeeds {
    /// Some selector uses `select=plural` (the default) → `plural.cardinal`.
    pub cardinal: bool,
    /// Some selector uses `select=ordinal` → `plural.ordinal`.
    pub ordinal: bool,
    /// The number entries.
    pub numbers: NumberNeeds,
}

/// The LOCALE entries a catalog for `locale` carries under `needs`, sorted by
/// key, as `mf2_catalog::writer::Options::locale_entries` takes them;
/// `plural.cardinal` also when units or currency names need it.
#[doc(hidden)]
pub fn locale_entries(locale: &str, needs: &LocaleNeeds) -> Result<Vec<(u32, Vec<u8>)>, Error> {
    let mut kinds = Vec::new();
    if needs.cardinal || needs.numbers.needs_cardinal() {
        kinds.push(PluralKind::Cardinal);
    }
    if needs.ordinal {
        kinds.push(PluralKind::Ordinal);
    }
    let mut out = plural_locale_entries(locale, &kinds)?;
    out.extend(number_locale_entries(locale, &needs.numbers)?);
    out.sort_by_key(|(k, _)| *k);
    Ok(out)
}
