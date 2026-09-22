//! `mf2-locale-data` — the locale data mf2-two catalogs carry
//! (`plans/05-tooling.md` §7; `plans/02-catalog-format.md` §4), build side
//! only: never linked into a client.
//!
//! Phase 3 builds the **plural** part and each locale's **text direction**
//! (from CLDR's likely scripts): the UTS #35 rule parser, the
//! canonical encoder of the `plural.cardinal` / `plural.ordinal` LOCALE
//! entries, and CLDR's rules for every locale, shipped in `data/` (an
//! application's `build.rs` has no `third_party/` and downloads nothing).
//! The CLDR `@integer`/`@decimal` samples of every locale are tests of the
//! client evaluator (`tests/cldr_samples.rs`). Number, currency and unit data
//! follow in Phase 4.

mod direction;
pub mod error;
#[cfg(feature = "extract")]
pub mod extract;
pub mod plural;

pub use direction::direction;
pub use error::{Error, ParseError};
pub use mf2_catalog::CldrVersion;
pub use plural::{
    LocaleRules, PluralKind, plural_entry, plural_locale_entries, plural_locales, plural_rules,
};

/// The CLDR release the shipped data comes from (`third_party/cldr-json/PIN`).
pub const CLDR_VERSION: CldrVersion = CldrVersion {
    major: 48,
    minor: 2,
    patch: 1,
};
