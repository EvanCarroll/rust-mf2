//! P0.4 build side (std): UTS #35 plural-rule parser, CLDR sample expansion,
//! encoder into the `plural.*` LOCALE entries, an independent reference
//! evaluator, and the all-locales correctness run. Throwaway probe code; the
//! product home is `mf2-locale-data` (plans/05-tooling.md §7).

pub mod check;
pub mod cldr;
pub mod encode;
pub mod error;
pub mod reference;
pub mod rule;
pub mod samples;

pub use error::{Error, ParseError};
