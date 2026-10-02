//! `mf2-host-std` — the [`Host`] of `mf2-runtime` for native code and for the
//! `wasm32-wasip1`: NFC
//! through `unicode-normalization` and the shortest round-trip text of a
//! float through `ryu`. Never
//! linked into the browser client, which uses `mf2-host-web`.
//!
//! Two hosts, so that a build without dates carries no time-zone database:
//!
//! | static | feature | adds |
//! |---|---|---|
//! | [`HOST`] | — | NFC and float text; a named zone is *Bad Option* |
//! | [`ZONES_HOST`] | `time-zones` | the UTC offset of a named zone through `jiff`'s **bundled** IANA database — never the system's, so every server, test and `wasm32-wasip1` run answers alike |
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.
//! An application reaches this crate through
//! [`mf2`](https://docs.rs/mf2), as `mf2::host_std` (feature `host-std`).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;

use mf2_runtime::Host;
use unicode_normalization::UnicodeNormalization;

/// The native host: NFC and float text.
#[derive(Clone, Copy, Default, Debug)]
pub struct StdHost;

/// The native host, for [`mf2_runtime::FormatContext::new`]. Its
/// `zone_offset` is the trait's default, so a named time zone is *Bad
/// Option*; a build with dates uses [`ZONES_HOST`].
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

/// The native host with time zones: [`StdHost`]'s NFC and float text, and
/// the UTC offset of a named time zone through `jiff`.
#[cfg(feature = "time-zones")]
#[cfg_attr(docsrs, doc(cfg(feature = "time-zones")))]
#[derive(Clone, Copy, Default, Debug)]
pub struct ZonesStdHost;

/// The native host with time zones, for
/// [`mf2_runtime::FormatContext::new`]: what a build with dates formats
/// through.
#[cfg(feature = "time-zones")]
#[cfg_attr(docsrs, doc(cfg(feature = "time-zones")))]
pub static ZONES_HOST: ZonesStdHost = ZonesStdHost;

#[cfg(feature = "time-zones")]
impl Host for ZonesStdHost {
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str {
        StdHost.nfc(s, buf)
    }

    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        StdHost.f64_to_text(x, buf)
    }

    /// The offset of `zone` (an IANA name, looked up case-insensitively, or
    /// one of jiff's special zones such as `UTC`) at `epoch_ms`, from the
    /// bundled database; else, when `zone` is a POSIX TZ rule
    /// (`TimeZone::rules`: a native application's system zone without an
    /// IANA name), from that rule. `None` for anything else, or an instant
    /// outside jiff's range (years −9999 to 9999).
    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        let t = jiff::Timestamp::from_millisecond(epoch_ms).ok()?;
        let tz = match jiff::tz::TimeZoneDatabase::bundled().get(zone) {
            Ok(tz) => tz,
            Err(_) => jiff::tz::TimeZone::posix(zone).ok()?,
        };
        Some(tz.to_offset(t).seconds())
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use mf2_runtime::Host;

    use super::HOST;
    #[cfg(feature = "time-zones")]
    use super::ZONES_HOST;

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
    #[cfg(feature = "time-zones")]
    fn at(year: i32, month: u8, day: u8, hour: i64, minute: i64, second: i64) -> i64 {
        let days =
            mf2_runtime::Date::new(year, month, day).map_or(0, mf2_runtime::Date::days_since_epoch);
        ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000
    }

    #[cfg(feature = "time-zones")]
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
            assert_eq!(ZONES_HOST.zone_offset(zone, t), Some(want), "{zone} at {t}");
        }
        assert_eq!(ZONES_HOST.zone_offset("Mars/Olympus_Mons", 0), None);
        assert_eq!(ZONES_HOST.zone_offset("", 0), None);
        assert_eq!(ZONES_HOST.zone_offset("EST 5", 0), None);
        // Outside jiff's range.
        assert_eq!(ZONES_HOST.zone_offset("Europe/Paris", i64::MAX), None);
    }

    /// `HOST` has no zone data at all: every name is *Bad Option*.
    #[test]
    fn plain_host_has_no_zones() {
        assert_eq!(HOST.zone_offset("Europe/Paris", 0), None);
        assert_eq!(HOST.zone_offset("UTC", 0), None);
    }

    /// A POSIX TZ rule, which is not a name the database has, follows its
    /// own changes of offset: the zone a native application's system has
    /// when it has no IANA name (`TZ=EST5EDT,M3.2.0,M11.1.0`).
    #[cfg(feature = "time-zones")]
    #[test]
    fn posix_rules() {
        const H: i32 = 3600;
        let us = "EST5EDT,M3.2.0,M11.1.0";
        let eu = "CET-1CEST,M3.5.0,M10.5.0/3";
        for (zone, t, want) in [
            // The second Sunday of March 2026, 02:00 local: 07:00Z.
            (us, at(2026, 3, 8, 6, 59, 59), -5 * H),
            (us, at(2026, 3, 8, 7, 0, 0), -4 * H),
            // The first Sunday of November 2026, 02:00 local daylight: 06:00Z.
            (us, at(2026, 11, 1, 5, 59, 59), -4 * H),
            (us, at(2026, 11, 1, 6, 0, 0), -5 * H),
            (us, at(2026, 1, 15, 17, 0, 0), -5 * H),
            (us, at(2026, 7, 15, 16, 0, 0), -4 * H),
            // The last Sunday of March 2026, 02:00 local: 01:00Z.
            (eu, at(2026, 3, 29, 0, 59, 59), H),
            (eu, at(2026, 3, 29, 1, 0, 0), 2 * H),
            // No daylight saving time at all.
            ("JST-9", at(2026, 7, 15, 0, 0, 0), 9 * H),
        ] {
            assert_eq!(ZONES_HOST.zone_offset(zone, t), Some(want), "{zone} at {t}");
        }
    }
}
