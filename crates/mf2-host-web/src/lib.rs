//! `mf2-host-web` — the [`Host`] of `mf2-runtime` in the browser
//! (`plans/03-runtime.md` §2.5, §4): NFC through `String.prototype.normalize`
//! and the shortest round-trip text of a float through `Number.prototype
//! .toString`, both via `js-sys`, so the wasm carries no normalization
//! tables and no float-printing code.
//!
//! Numbers (`intl`, `plans/03-runtime.md` §2.7, §5.3) come as another host,
//! not as more methods of [`HOST`]: a host method is linked whenever its
//! host is (it is in the `Host` vtable), so a number method on [`HOST`]
//! would cost every client that has the feature on, numbers or not (B1′).
//! An application names the host its corpus needs (`mf2-build` picks it):
//!
//! | Static | Feature | Adds |
//! |---|---|---|
//! | [`HOST`] | — | NFC, float text |
//! | `NUMBERS_HOST`, `IntlNumbers(&host)` | `intl` | numbers through `Intl.NumberFormat` and `Intl.PluralRules` (`Host::numbers`) over another host (`numbers.rs`) |

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "intl")]
mod numbers;

use alloc::string::String;

use js_sys::{JsString, Number};
use mf2_runtime::Host;

#[cfg(feature = "intl")]
pub use numbers::{IntlNumbers, NUMBERS_HOST};

/// The browser host.
#[derive(Clone, Copy, Default, Debug)]
pub struct WebHost;

/// The browser host, for [`mf2_runtime::FormatContext::new`].
pub static HOST: WebHost = WebHost;

impl Host for WebHost {
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str {
        let normalized = JsString::from(s).normalize("NFC");
        buf.clear();
        match normalized.as_string() {
            Some(text) => {
                *buf = text;
                buf
            }
            None => s,
        }
    }

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
