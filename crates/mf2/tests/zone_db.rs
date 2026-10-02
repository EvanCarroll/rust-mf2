//! Which IANA database a named time zone is looked up in (`plan/01` §4.1,
//! owner decision 3): the machine's, so a native application follows the
//! system it runs on, or jiff's bundled copy where the build asked for it
//! with `tzdb-bundled` (which `ssr` and `axum` turn on, so that every reply
//! says the same thing whatever the host holds).
//!
//! The test supplies a database of its own — one hand-written `TZif` file in a
//! directory it creates — and names it in `TZDIR`. jiff reads `TZDIR` once,
//! when it first looks a zone up, so the assertions run in this binary
//! started again with that variable set, as `tests/system_zone.rs` does for
//! `TZ`.
//!
//! Each build then sees exactly one of the two databases, which is what the
//! two halves below assert: without `tzdb-bundled` the supplied zone is
//! found and a real IANA name is *Bad Option*, and with it the supplied
//! directory is ignored and the real name answers from the bundle.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mf2::Host;
use mf2::host_std::ZONES_HOST;

/// The zone the supplied database holds, and its fixed offset: +02:30, a
/// value no IANA zone of that name could be confused with.
const SUPPLIED: &str = "Test/Supplied";
const SUPPLIED_OFFSET: i32 = 2 * 3600 + 1800;

/// A zone every real IANA database holds and the supplied one does not.
const REAL: &str = "Europe/Paris";
/// 2006-01-02T00:00:00Z, winter in Paris: +01:00.
const WINTER_MS: i64 = 1_136_160_000_000;

const CHILD: &str = "MF2_ZONE_DB_CHILD";

#[test]
fn a_lookup_reads_the_database_this_build_asked_for() {
    let dir = supplied_database();
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "in_a_process_whose_tzdir_is_supplied",
            "--nocapture",
        ])
        .env("TZDIR", &dir)
        .env(CHILD, "1")
        .status()
        .expect("the test binary runs again");
    fs::remove_dir_all(dir.parent().expect("the test's own directory")).ok();
    assert!(
        status.success(),
        "the child's assertions failed: see its output above"
    );
}

/// Runs only in the child that the test above starts.
#[test]
fn in_a_process_whose_tzdir_is_supplied() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    // Neither build may panic on a name its database does not hold: the
    // formatter turns `None` into *Bad Option*.
    assert_eq!(ZONES_HOST.zone_offset("Mars/Olympus_Mons", 0), None);

    #[cfg(not(feature = "tzdb-bundled"))]
    {
        assert_eq!(
            ZONES_HOST.zone_offset(SUPPLIED, WINTER_MS),
            Some(SUPPLIED_OFFSET),
            "the system's database is the one in `TZDIR`"
        );
        assert_eq!(
            ZONES_HOST.zone_offset(REAL, WINTER_MS),
            None,
            "a zone the machine's database does not hold is *Bad Option*"
        );
    }

    #[cfg(feature = "tzdb-bundled")]
    {
        assert_eq!(
            ZONES_HOST.zone_offset(REAL, WINTER_MS),
            Some(3600),
            "the bundle answers whatever `TZDIR` holds"
        );
        assert_eq!(
            ZONES_HOST.zone_offset(SUPPLIED, WINTER_MS),
            None,
            "the bundle holds no zone of the test's own"
        );
    }
}

/// Writes the database the test supplies and returns its root, which becomes
/// the child's `TZDIR`.
fn supplied_database() -> PathBuf {
    let root = std::env::temp_dir().join(format!("mf2-zone-db-{}", std::process::id()));
    let dir = root.join("zoneinfo");
    let (region, city) = SUPPLIED.split_once('/').expect("a two-part zone name");
    fs::create_dir_all(dir.join(region)).expect("the supplied database's directory");
    write_tzif(&dir.join(region).join(city), SUPPLIED_OFFSET, "TST");
    dir
}

/// One `TZif` file (RFC 8536, version 2) for a zone whose offset never
/// changes: no transitions, one local-time type, and a POSIX TZ footer
/// saying the same thing.
fn write_tzif(path: &Path, offset: i32, abbreviation: &str) {
    let mut chars = abbreviation.as_bytes().to_vec();
    chars.push(0);
    let mut bytes = Vec::new();
    // The 32-bit block, then the 64-bit one: identical here, as neither has
    // a transition to record.
    for _ in 0..2 {
        bytes.extend_from_slice(b"TZif2");
        bytes.extend_from_slice(&[0u8; 15]);
        // isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt.
        let charcnt = u32::try_from(chars.len()).expect("a short abbreviation");
        for count in [0u32, 0, 0, 0, 1, charcnt] {
            bytes.extend_from_slice(&count.to_be_bytes());
        }
        bytes.extend_from_slice(&offset.to_be_bytes()); // utoff
        bytes.push(0); // not daylight saving time
        bytes.push(0); // the first abbreviation
        bytes.extend_from_slice(&chars);
    }
    // POSIX TZ, whose sign is the other way round: `-2:30` is UTC+02:30.
    let hours = offset / 3600;
    let minutes = (offset % 3600) / 60;
    bytes.extend_from_slice(format!("\n{abbreviation}-{hours}:{minutes:02}\n").as_bytes());
    fs::write(path, &bytes).expect("the supplied TZif file");
}
