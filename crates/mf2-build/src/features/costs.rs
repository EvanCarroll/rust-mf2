//! What each formatter costs, as `cargo xtask feature-costs` measured it.
//!
//! Written by that command beside `docs/feature-costs.md`, and held to that
//! table by `cargo xtask ci`: do not edit by hand. A browser's figure is
//! brotli bytes of the client wasm, native code's bytes of the stripped
//! binary; each is the size with the feature less the size without it,
//! against the build the table names.

/// How a browser's figure is counted, as the message that quotes it says
/// it. Written here, beside the figures, so that a regenerated table
/// cannot leave the wording of an older unit behind.
pub(super) const BROWSER_UNIT: &str = " brotli";

/// `leptos-client-number-builtin`, set against `leptos-client-number-plain`.
pub(super) const NUMBER_BUILTIN_BROWSER: i64 = 2_081;

/// `leptos-client-number-intl`, set against `leptos-client-number-plain`.
pub(super) const NUMBER_INTL_BROWSER: i64 = -152;

/// `leptos-client-datetime-iso`, set against `leptos-client-number-builtin`.
pub(super) const DATE_ISO_BROWSER: i64 = 3_396;

/// `leptos-client-datetime-icu`, set against `leptos-client-number-builtin`, `leptos-client-datetime-iso`.
pub(super) const DATE_ICU_BROWSER: i64 = 45_245;

/// `leptos-client-datetime-icu-cached`, set against `leptos-client-number-builtin`, `leptos-client-datetime-icu`.
pub(super) const DATE_ICU_CACHED_BROWSER: i64 = 1_396;

/// `leptos-client-datetime-intl`, set against `leptos-client-number-builtin`, `leptos-client-datetime-iso`.
pub(super) const DATE_INTL_BROWSER: i64 = 250;

/// `native-number-builtin`, set against `native`, `native-number-plain`.
pub(super) const NUMBER_BUILTIN_NATIVE: i64 = 9_696;

/// `native-datetime-iso`, set against `native`, `native-number-plain`.
pub(super) const DATE_ISO_NATIVE: i64 = 167_648;

/// `native-datetime-icu`, set against `native`, `native-number-plain`.
pub(super) const DATE_ICU_NATIVE: i64 = 330_160;
