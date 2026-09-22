//! `mf2-host-std` — the [`Host`] of `mf2-runtime` for native code and for the
//! `wasm32-wasip1` conformance run (`plans/03-runtime.md` §2.5, §4): NFC
//! through `unicode-normalization`, the shortest round-trip text of a float
//! through `ryu`. Never linked into the browser client, which uses
//! `mf2-host-web`.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;

use mf2_runtime::Host;
use unicode_normalization::UnicodeNormalization;

/// The native host.
#[derive(Clone, Copy, Default, Debug)]
pub struct StdHost;

/// The native host, for [`mf2_runtime::FormatContext::new`].
pub static HOST: StdHost = StdHost;

impl Host for StdHost {
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str {
        buf.clear();
        buf.extend(s.nfc());
        buf
    }

    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        if !x.is_finite() {
            return None;
        }
        let mut b = ryu::Buffer::new();
        let text = b.format_finite(x).as_bytes();
        let out = buf.get_mut(..text.len())?;
        out.copy_from_slice(text);
        core::str::from_utf8(out).ok()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use mf2_runtime::Host;

    use super::HOST;

    #[test]
    fn nfc() {
        let mut buf = String::new();
        assert_eq!(HOST.nfc("\u{1E0A}\u{0323}", &mut buf), "\u{1E0C}\u{0307}");
        let mut buf = String::new();
        assert_eq!(HOST.nfc("e\u{301}", &mut buf), "\u{e9}");
    }

    #[test]
    fn floats() {
        let mut buf = [0u8; 32];
        for (x, want) in [
            (4.2, "4.2"),
            (1.0, "1.0"),
            (-0.5, "-0.5"),
            (1e21, "1e21"),
            (1.5e-7, "1.5e-7"),
            (f64::MAX, "1.7976931348623157e308"),
            (-f64::MIN_POSITIVE, "-2.2250738585072014e-308"),
        ] {
            assert_eq!(HOST.f64_to_text(x, &mut buf), Some(want));
        }
        assert_eq!(HOST.f64_to_text(f64::NAN, &mut buf), None);
    }
}
