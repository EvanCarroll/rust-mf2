//! `intl-probe-fn` — Phase 4 task A0 (`plans/11-phase-4-work-order.md`,
//! "The `intl` client option"): the numeric functions over the browser's
//! `Intl`, as `plans/03-runtime.md` §5.3 designs them. **Rust keeps MF2's
//! semantics** — option validation and MF2's errors (`Intl` throws where
//! MF2 reports *Bad Option* and continues), the operand rules, exact-match
//! keys, `select`'s literal-only rule and option inheritance, `:offset`'s
//! exact decimal addition — while **`Intl.NumberFormat` rounds and writes
//! the digits** (the exact decimal passed as a string, with the resolved
//! digit options; `useGrouping: false` and `numberingSystem: "latn"` for
//! neutral output) and **`Intl.PluralRules`** (the same digit options) gives
//! the plural category.
//!
//! | Static | Function | Output |
//! |---|---|---|
//! | [`NUMBER`], [`INTEGER`], [`OFFSET`] | `:number`, `:integer`, `:offset` | neutral (ASCII digits, `.`, `-`/`+`, no grouping): today's Rust output |
//! | [`NUMBER_LOC`], [`INTEGER_LOC`], [`OFFSET_LOC`], [`PERCENT`] | the same with the catalog locale's symbols and grouping; `:percent` | localized |
//! | [`CURRENCY`], [`UNIT`] | `:currency`, `:unit` | localized |
//!
//! The calls into `Intl` are the `Host` methods the option would add; they
//! live in the probe-local `host` module (`src/host.rs`) (the probe does not change
//! `mf2_runtime::Host`). The handlers implement the public
//! [`mf2_runtime::Function`] trait and link none of the runtime's numeric
//! code (its digit plan, rounding, display and plural evaluator are
//! `pub(crate)` there; the option code this crate needs is ported, see
//! `options.rs`).
//!
//! Client-path rules (as `mf2-runtime`): `no_std` + `alloc`,
//! `forbid(unsafe_code)`, no `format!` / `Debug` / `Display`, no `unwrap` or
//! panicking indexing.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

extern crate alloc;

mod host;
mod measure;
mod number;
mod options;
mod text;

pub use number::IntlNumber;

/// `:number`, neutral output.
pub static NUMBER: IntlNumber = IntlNumber::number(false);
/// `:integer`, neutral output.
pub static INTEGER: IntlNumber = IntlNumber::integer(false);
/// `:offset`, neutral output.
pub static OFFSET: IntlNumber = IntlNumber::offset(false);

/// `:number` with the catalog locale's symbols.
pub static NUMBER_LOC: IntlNumber = IntlNumber::number(true);
/// `:integer` with the catalog locale's symbols.
pub static INTEGER_LOC: IntlNumber = IntlNumber::integer(true);
/// `:offset` with the catalog locale's symbols.
pub static OFFSET_LOC: IntlNumber = IntlNumber::offset(true);
/// `:percent`.
pub static PERCENT: IntlNumber = IntlNumber::percent();

/// `:currency`.
pub static CURRENCY: IntlNumber = IntlNumber::currency();
/// `:unit`.
pub static UNIT: IntlNumber = IntlNumber::unit();
