//! `mf2-host-web` — the [`Host`] of `mf2-runtime` in the browser
//! (`wasm32-unknown-unknown`, its only target): the shortest round-trip text
//! of a float through `Number.prototype.toString`, via `js-sys`, so the wasm
//! carries no float-printing code. Canonical equivalence is the runtime's
//! own, from the catalog's map, so there is no `normalize` glue either.
//!
//! Dates and numbers come as
//! more hosts, not as more methods of [`HOST`]: a host method is linked
//! whenever its host is (it is in the `Host` vtable), so a date or number
//! method on [`HOST`] would cost every client that has the feature on,
//! dates or numbers or not. An application names the host its corpus
//! needs (`mf2-build` picks it):
//!
//! | Static | Feature | Adds |
//! |---|---|---|
//! | [`HOST`] | — | float text |
//! | [`ZONES_HOST`] | `time-zones` | `Host::zone_offset` from the browser's zone data (clients of the `icu` date formatter: named zones) |
//! | [`INTL_HOST`] | `datetime-intl` | that, and `Host::format_date_time` through `Intl.DateTimeFormat` |
//! | `NUMBERS_HOST`, `IntlNumbers(&host)` | `number-intl` | numbers through `Intl.NumberFormat` and `Intl.PluralRules` (`Host::numbers`) over another host |
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.
//! An application reaches this crate through
//! [`mf2`](https://docs.rs/mf2), as `mf2::host_web` (feature `host-web`).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "time-zones")]
mod dates;
#[cfg(feature = "number-intl")]
mod numbers;

use js_sys::Number;
use mf2_runtime::Host;

#[cfg(all(feature = "number-intl", feature = "datetime-intl"))]
pub use numbers::INTL_DATES_NUMBERS_HOST;
#[cfg(all(feature = "number-intl", feature = "time-zones"))]
pub use numbers::ZONES_NUMBERS_HOST;
#[cfg(feature = "number-intl")]
pub use numbers::{IntlNumbers, NUMBERS_HOST};

/// The browser host.
#[derive(Clone, Copy, Default, Debug)]
pub struct WebHost;

/// The browser host, for [`mf2_runtime::FormatContext::new`].
pub static HOST: WebHost = WebHost;

/// The browser host with time-zone offsets from the browser's zone data.
#[cfg(feature = "time-zones")]
#[derive(Clone, Copy, Default, Debug)]
pub struct ZonesWebHost;

/// [`ZonesWebHost`], for [`mf2_runtime::FormatContext::new`].
#[cfg(feature = "time-zones")]
pub static ZONES_HOST: ZonesWebHost = ZonesWebHost;

/// The browser host with dates through `Intl.DateTimeFormat` and zone
/// offsets from the browser's zone data (`datetime-intl`).
#[cfg(feature = "datetime-intl")]
#[derive(Clone, Copy, Default, Debug)]
pub struct IntlWebHost;

/// [`IntlWebHost`], for [`mf2_runtime::FormatContext::new`].
#[cfg(feature = "datetime-intl")]
pub static INTL_HOST: IntlWebHost = IntlWebHost;

impl Host for WebHost {
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        if !x.is_finite() {
            return None;
        }
        let text = Number::from(x).to_string_with_radix(10).ok()?.as_string()?;
        let out = buf.get_mut(..text.len())?;
        out.copy_from_slice(text.as_bytes());
        core::str::from_utf8(out).ok()
    }
}

#[cfg(feature = "time-zones")]
impl Host for ZonesWebHost {
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        WebHost.f64_to_text(x, buf)
    }

    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        dates::zone_offset(zone, epoch_ms)
    }
}

#[cfg(feature = "datetime-intl")]
impl Host for IntlWebHost {
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        WebHost.f64_to_text(x, buf)
    }

    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        dates::zone_offset(zone, epoch_ms)
    }

    fn format_date_time(
        &self,
        locale: &str,
        request: &mf2_runtime::DateTimeRequest<'_>,
        out: &mut dyn mf2_runtime::Sink,
    ) -> bool {
        dates::format(locale, request, out)
    }
}
