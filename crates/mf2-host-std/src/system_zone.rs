//! The machine's own time zone: a platform service, as the offset of a
//! named zone is, so it belongs with the host rather than in `mf2`
//! (`plan/01` §4.1). A native application shows its dates in it: by its
//! IANA name; without one, the rules it follows; UTC as the last resort.
//! Never the offset in force when the application started, which the next
//! change of offset would leave behind (1.x's behaviour).

use std::path::PathBuf;
use std::sync::OnceLock;

use mf2_runtime::TimeZone;

/// The time zone the machine is set to, read once per process: its IANA
/// name where it has one, else the daylight-saving rules it follows (which
/// [`ZONES_HOST`](crate::ZONES_HOST) evaluates), else UTC. What
/// `mf2::native` gives a native application's dates unless the program sets
/// another.
#[cfg_attr(docsrs, doc(cfg(feature = "time-zones")))]
#[must_use]
pub fn system_time_zone() -> TimeZone {
    static SYSTEM: OnceLock<TimeZone> = OnceLock::new();
    *SYSTEM.get_or_init(read)
}

/// jiff's answer, by its IANA name; else the POSIX TZ rule the zone
/// follows ([`TimeZone::rules`], which [`ZONES_HOST`](crate::ZONES_HOST)
/// evaluates); else UTC.
fn read() -> TimeZone {
    if let Ok(zone) = jiff::tz::TimeZone::try_system()
        && let Some(named) = zone.iana_name().and_then(TimeZone::named)
    {
        return named;
    }
    rules().unwrap_or(TimeZone::UTC)
}

/// The rule of a system zone that has no IANA name: `TZ` itself when it
/// holds a POSIX rule (`EST5EDT,M3.2.0,M11.1.0`), else the rule the zone's
/// file ends with — `TZ` when it names a file, else `/etc/localtime`, read
/// as a `TZif` file (RFC 8536), whose version 2 and later end with the rule
/// for the times past their table. Each rule is one jiff reads.
fn rules() -> Option<TimeZone> {
    let tz = std::env::var("TZ").ok();
    let file = match tz.as_deref().map(|v| v.strip_prefix(':').unwrap_or(v)) {
        Some(value) if !value.is_empty() => {
            if jiff::tz::TimeZone::posix(value).is_ok() {
                return TimeZone::rules(value);
            }
            if !value.starts_with('/') {
                // A name jiff does not know: nothing to read.
                return None;
            }
            PathBuf::from(value)
        }
        _ => PathBuf::from("/etc/localtime"),
    };
    let bytes = std::fs::read(file).ok()?;
    let rule = footer(&bytes)?;
    jiff::tz::TimeZone::posix(rule).ok()?;
    TimeZone::rules(rule)
}

/// The rule a `TZif` file of version 2 or later ends with: the text between
/// its last two line feeds, when there is any.
fn footer(bytes: &[u8]) -> Option<&str> {
    if !bytes.starts_with(b"TZif") || bytes.get(4).is_none_or(|v| *v < b'2') {
        return None;
    }
    let body = bytes.strip_suffix(b"\n")?;
    let start = body.iter().rposition(|&b| b == b'\n')? + 1;
    core::str::from_utf8(body.get(start..)?)
        .ok()
        .filter(|rule| !rule.is_empty())
}

#[cfg(test)]
mod tests {
    use super::footer;

    #[test]
    fn a_tzif_file_ends_with_its_rule() {
        let mut file = b"TZif2".to_vec();
        file.extend_from_slice(&[0; 40]);
        file.extend_from_slice(b"\nEST5EDT,M3.2.0,M11.1.0\n");
        assert_eq!(footer(&file), Some("EST5EDT,M3.2.0,M11.1.0"));
        // A version 1 file has none, nor has a file without the final line
        // feed, an empty rule, or something that is not a TZif file.
        let mut old = b"TZif\0".to_vec();
        old.extend_from_slice(&[0; 40]);
        old.extend_from_slice(b"\nEST5EDT,M3.2.0,M11.1.0\n");
        assert_eq!(footer(&old), None);
        file.pop();
        assert_eq!(footer(&file), None);
        assert_eq!(footer(b"TZif3...\n\n"), None);
        assert_eq!(footer(b"#!/bin/sh\nEST5EDT\n"), None);
    }
}
