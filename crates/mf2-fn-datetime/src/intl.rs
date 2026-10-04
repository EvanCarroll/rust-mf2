//! The `web-intl` backend in the browser:
//! the plan goes to the host's date formatter, `Host::format_date_time` —
//! `mf2-host-web`'s `Intl.DateTimeFormat` — as a `DateTimeRequest`
//! ([`Plan::request`]), so the wasm carries no date-formatting code and the
//! catalog no date data. Server text (ICU4X, CLDR 48) and browser text can
//! differ cosmetically (P0.10: harmless to hydration).

use mf2_runtime::{Dir, FnContext, FormatError, Sink};

use crate::neutral::Neutral;
use crate::plan::{Backend, Plan};

/// The host's date formatter (`Host::format_date_time`). A host without one
/// (it answers `false`, the default: an application that names
/// `mf2_host_web::HOST` rather than `INTL_HOST`), or one that rejects the
/// request, gets the neutral text.
#[derive(Clone, Copy, Default, Debug)]
pub struct Intl;

/// ECMA-402's time range: ±8.64 × 10¹⁵ ms around the epoch (±100,000,000
/// days).
const MAX_MS: i64 = 8_640_000_000_000_000;

impl Backend for Intl {
    /// A plan outside ECMA-402's time range is an *Unsupported Operation*
    /// (the ICU4X backend's range is narrower still).
    fn supports(&self, _cx: &FnContext<'_>, plan: &Plan<'_>) -> Result<(), FormatError> {
        if plan.request().epoch_ms.unsigned_abs() > MAX_MS.unsigned_abs() {
            return Err(FormatError::UnsupportedOperation);
        }
        Ok(())
    }

    fn format(&self, cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
        if !cx
            .host()
            .format_date_time(cx.locale(), &plan.request(), out)
        {
            Neutral.format(cx, plan, out);
        }
    }

    /// Localized text has the catalog's direction.
    fn dir(&self, cx: &FnContext<'_>, _plan: &Plan<'_>) -> Dir {
        cx.catalog().dir()
    }
}
