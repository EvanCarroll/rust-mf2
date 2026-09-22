//! `mf2-host-std` — the [`Host`] of `mf2-runtime` for native code and for the
//! `wasm32-wasip1` conformance run (`plans/03-runtime.md` §2.5, §4): NFC
//! through `unicode-normalization`, the shortest round-trip text of a float
//! through `ryu`, and the UTC offset of a named time zone through `jiff`'s
//! **bundled** IANA database (owner decision 1, §5.2) — never the system's,
//! so a server, a test and the `wasm32-wasip1` run answer alike. Never
//! linked into the browser client, which uses `mf2-host-web`.

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

    /// The offset of `zone` (an IANA name, looked up case-insensitively, or
    /// one of jiff's special zones such as `UTC`) at `epoch_ms`, from the
    /// bundled database; `None` for an unknown zone or an instant outside
    /// jiff's range (years −9999 to 9999).
    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        let tz = jiff::tz::TimeZoneDatabase::bundled().get(zone).ok()?;
        let t = jiff::Timestamp::from_millisecond(epoch_ms).ok()?;
        Some(tz.to_offset(t).seconds())
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

    /// Milliseconds since the epoch of `year-month-day hour:minute:second` UTC.
    fn at(year: i32, month: u8, day: u8, hour: i64, minute: i64, second: i64) -> i64 {
        let days =
            mf2_runtime::Date::new(year, month, day).map_or(0, mf2_runtime::Date::days_since_epoch);
        ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000
    }

    #[test]
    fn zone_offsets() {
        const H: i32 = 3600;
        for (zone, t, want) in [
            // EU summer time, 2006: 2006-03-26 01:00Z to 2006-10-29 01:00Z.
            ("Europe/Paris", at(2006, 3, 26, 0, 59, 59), H),
            ("Europe/Paris", at(2006, 3, 26, 1, 0, 0), 2 * H),
            ("Europe/Paris", at(2006, 10, 29, 0, 59, 59), 2 * H),
            ("Europe/Paris", at(2006, 10, 29, 1, 0, 0), H),
            // US since 2007: 2021-03-14 07:00Z and 2021-11-07 06:00Z.
            ("America/New_York", at(2021, 3, 14, 6, 59, 59), -5 * H),
            ("America/New_York", at(2021, 3, 14, 7, 0, 0), -4 * H),
            ("America/New_York", at(2021, 11, 7, 5, 59, 59), -4 * H),
            ("America/New_York", at(2021, 11, 7, 6, 0, 0), -5 * H),
            // Before 2007 the US rules differed: 2006-04-02 07:00Z.
            ("America/New_York", at(2006, 3, 20, 12, 0, 0), -5 * H),
            ("America/New_York", at(2006, 4, 2, 7, 0, 0), -4 * H),
            // Southern hemisphere: Sydney is on summer time in January.
            ("Australia/Sydney", at(2006, 1, 2, 0, 0, 0), 11 * H),
            ("Australia/Sydney", at(2006, 7, 2, 0, 0, 0), 10 * H),
            ("Asia/Kolkata", at(2006, 1, 2, 0, 0, 0), 5 * H + 1800),
            ("Asia/Kathmandu", at(2006, 1, 2, 0, 0, 0), 5 * H + 2700),
            ("UTC", at(2006, 1, 2, 0, 0, 0), 0),
            ("Etc/GMT+5", at(2006, 1, 2, 0, 0, 0), -5 * H),
            // Case-insensitive, as IANA lookups are.
            ("europe/paris", at(2006, 1, 2, 0, 0, 0), H),
            // Paris mean time before 1911: an offset with seconds.
            ("Europe/Paris", at(1900, 1, 1, 0, 0, 0), 561),
        ] {
            assert_eq!(HOST.zone_offset(zone, t), Some(want), "{zone} at {t}");
        }
        assert_eq!(HOST.zone_offset("Mars/Olympus_Mons", 0), None);
        assert_eq!(HOST.zone_offset("", 0), None);
        // Outside jiff's range.
        assert_eq!(HOST.zone_offset("Europe/Paris", i64::MAX), None);
    }
}
