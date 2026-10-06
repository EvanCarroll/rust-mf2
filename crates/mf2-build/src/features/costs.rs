//! What each formatter costs, as `cargo xtask feature-costs` measured it.
//!
//! Written by that command beside `docs/feature-costs.md`, and held to that
//! table by `cargo xtask ci`: do not edit by hand. A browser's figure is
//! gzip bytes of the client wasm, native code's bytes of the stripped
//! binary; each is the size with the feature less the size without it,
//! against the build the table names.

/// How a browser's figure is counted, as the message that quotes it says
/// it. Written here, beside the figures, so that a regenerated table
/// cannot leave the wording of an older unit behind.
pub(super) const BROWSER_UNIT: &str = " gzip";

/// `leptos-client-number-builtin`, set against `leptos-client-number-plain`.
pub(super) const NUMBER_BUILTIN_BROWSER: i64 = 2_514;

/// `leptos-client-number-intl`, set against `leptos-client-number-plain`.
pub(super) const NUMBER_INTL_BROWSER: i64 = -545;

/// `leptos-client-datetime-iso`, set against `leptos-client-number-builtin`.
pub(super) const DATE_ISO_BROWSER: i64 = 5_336;

/// `leptos-client-datetime-icu`, set against `leptos-client-number-builtin`, `leptos-client-datetime-iso`.
pub(super) const DATE_ICU_BROWSER: i64 = 59_292;

/// `leptos-client-datetime-icu-cached`, set against `leptos-client-number-builtin`, `leptos-client-datetime-icu`.
pub(super) const DATE_ICU_CACHED_BROWSER: i64 = 2_231;

/// `leptos-client-datetime-intl`, set against `leptos-client-number-builtin`, `leptos-client-datetime-iso`.
pub(super) const DATE_INTL_BROWSER: i64 = 247;

/// `native-number-builtin`, set against `native`, `native-number-plain`.
pub(super) const NUMBER_BUILTIN_NATIVE: i64 = 9_920;

/// `native-datetime-iso`, set against `native`, `native-number-plain`.
pub(super) const DATE_ISO_NATIVE: i64 = 167_192;

/// `native-datetime-icu`, set against `native`, `native-number-plain`.
pub(super) const DATE_ICU_NATIVE: i64 = 328_664;
