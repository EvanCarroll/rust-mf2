//! `cargo xtask native-canaries`: what a feature set *links* into a native
//! binary (`plan/01` §6.1, guard rail 1).
//!
//! A size claim about a feature is only worth what the linker does with it.
//! For each row of [`ROWS`] this links `tools/native-canary` — the smallest
//! application that uses MF2: a typed language, a plain message, a number
//! and, with a date formatter, a date in a named time zone — in the profile
//! releases use and **unstripped**, then reads the binary's symbol table and
//! says which of [`CRATES`] still has a named item in it. A row that
//! requires a crate's symbols fails when there are none, and a row that
//! forbids them fails when there is one.
//!
//! The reader is `nm`, which every image that links a Rust binary has. A
//! symbol counts for the crate its *path begins with*, not for whichever
//! crate instantiated it: `core::ptr::drop_glue::<String>` compiled into
//! jiff's object is core's item, and a row that forbids jiff must not fail
//! on it.
//!
//! The report — which of the crates each row links, and the binary's size —
//! is printed on every run and written to `target/native-canaries/report.md`.
//! It is what answers `plan/01` §8, "`native` links by use".
//!
//! Phase 12 made jiff conditional (`mf2-host-std`'s feature `time-zones`,
//! which only a date formatter turns on) and removed `ryu` outright, so every
//! dateless set forbids both. Phase 13 did the same for
//! `unicode-normalization`: the runtime decides canonical equivalence from
//! the map its catalog carries (`plan/01` §4.3), so the tables are linked
//! only by a build that compiles messages at run time — which is why
//! `native,compile` is a row of its own, requiring them, beside the rows
//! that forbid them.
//!
//! One entry of [`CRATES`] is a module path rather than a crate:
//! jiff's bundled IANA database is its own `jiff::tz::db::bundled`, not a
//! separate crate in the symbol table, and whether a native application
//! reads the system's database or carries a copy (a quarter of a megabyte)
//! is exactly what a row should be able to say.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The crates whose presence every row reports: the heavy native
/// dependencies of `mf2-host-std`, `mf2`'s own `native` additions, and —
/// the one entry that is a module path, not a crate — jiff's bundled copy
/// of the IANA database.
const CRATES: &[&str] = &[
    "jiff",
    BUNDLED_TZDB,
    "unicode_normalization",
    "ryu",
    "sha2",
    "sys_locale",
];

/// The bundled IANA database, which lives in jiff's own
/// `jiff::tz::db::bundled` and so owns no crate name of its own.
const BUNDLED_TZDB: &str = "jiff::tz::db::bundled";

/// That module's path as both mangling schemes write it: v0 puts the crate
/// after a `Cs<hash>_` disambiguator, the legacy scheme after `_ZN`, and
/// both then write each component as a length and its bytes.
const BUNDLED_TZDB_PATH: &str = "4jiff2tz2db7bundled";

/// One row: a feature set of the canary application, and what its binary
/// must and must not carry.
struct Row {
    /// What the row is, in a line.
    what: &'static str,
    /// The canary crate's features, `--no-default-features` aside.
    features: &'static str,
    /// Crates that must have a symbol in the binary.
    requires: &'static [&'static str],
    /// Crates that must have none.
    forbids: &'static [&'static str],
}

/// Every row, one native binary each.
const ROWS: &[Row] = &[
    Row {
        what: "a native application with no formatting functions",
        features: "native",
        requires: &[],
        forbids: LEAN,
    },
    Row {
        what: "a native application formatting numbers",
        features: "native,fn-number",
        requires: &[],
        forbids: LEAN,
    },
    Row {
        what: "a terminal UI formatting numbers",
        features: "ratatui,fn-number",
        requires: &[],
        forbids: LEAN,
    },
    Row {
        // `axum` turns `tzdb-bundled` on, so that every reply says the same
        // thing whatever the host holds — but it does not turn a date formatter
        // on, and `tzdb-bundled` is weak in jiff. A server with no date in
        // any message therefore links no zone database at all.
        what: "an Axum server with no date in any message",
        features: "axum",
        requires: &[],
        forbids: LEAN,
    },
    Row {
        // The positive control, and the one row about which database a
        // lookup reads: a date in a named zone reaches
        // `Host::zone_offset`, which is jiff. If this row reported no jiff,
        // the reader would be broken and every forbidding row vacuous. The
        // zone comes from the system, so jiff's bundled copy of the IANA
        // database — a quarter of a megabyte — must not be linked.
        what: "a native application formatting a date in a named zone (the positive control)",
        features: "native,native-datetime-iso",
        requires: &["jiff"],
        forbids: &[BUNDLED_TZDB],
    },
    Row {
        // The other positive control: compiling a message at run time parses
        // it, and the parser normalizes names and keys. Without this row
        // every row that forbids the tables would pass even if the reader
        // had stopped seeing them.
        what: "a native application that compiles a message at run time (the positive control for the normalization tables)",
        features: "native,compile",
        requires: &[NFC_TABLES],
        forbids: &[],
    },
];

/// What a native application with prebuilt catalogs and no dates must not
/// link: the date library and its bundled database, the float writer
/// `StdHost::f64_to_text` stopped using in 12.4, and the normalization
/// tables, which 13.5 left to the build side and to `compile` alone.
const LEAN: &[&str] = &["jiff", BUNDLED_TZDB, "ryu", NFC_TABLES];

/// The normalization tables. A build that compiles a message at run time
/// parses it, which normalizes names and keys; nothing else links them.
const NFC_TABLES: &str = "unicode_normalization";

/// The canary application's binary.
const BIN: &str = "native-canary";

/// What one row's binary turned out to carry.
struct Linked {
    row: &'static Row,
    size: u64,
    /// The crates of [`CRATES`] with a symbol in it, in that order.
    present: Vec<&'static str>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let out = root.join("target/native-canaries");
    let manifest = root.join("tools/native-canary/Cargo.toml");
    let target = out.join("target");
    let mut rows = Vec::with_capacity(ROWS.len());
    for row in ROWS {
        eprintln!("native-canaries: linking `{}`: {}", row.features, row.what);
        build(root, &manifest, &target, row.features)?;
        let bin = target.join("release").join(BIN);
        let size = fs::metadata(&bin)
            .map_err(|source| Error::IoAt {
                path: bin.clone(),
                source,
            })?
            .len();
        let owners = owners(root, &bin)?;
        let present = CRATES
            .iter()
            .copied()
            .filter(|c| owners.contains(*c))
            .collect();
        rows.push(Linked { row, size, present });
    }

    let report = report(&rows);
    print!("{report}");
    fsx::write(&out.join("report.md"), report.as_bytes())?;
    let failures = judge(&rows);
    if !failures.is_empty() {
        return Err(Error::NativeCanaries(failures.join("; ")));
    }
    eprintln!(
        "native-canaries: {} feature sets linked, every row holds",
        rows.len()
    );
    Ok(())
}

/// What the canaries refuse: a required crate with no symbol, a forbidden
/// crate with one.
fn judge(rows: &[Linked]) -> Vec<String> {
    let mut failures = Vec::new();
    for linked in rows {
        for wanted in linked.row.requires {
            if !linked.present.contains(wanted) {
                failures.push(format!(
                    "`{}` links no {wanted} symbol, though the row requires one",
                    linked.row.features
                ));
            }
        }
        for unwanted in linked.row.forbids {
            if linked.present.contains(unwanted) {
                failures.push(format!(
                    "`{}` links {unwanted}, which the row forbids",
                    linked.row.features
                ));
            }
        }
    }
    failures
}

/// Links the canary application with `features` into `target`, in the
/// profile releases use and unstripped.
fn build(root: &Path, manifest: &Path, target: &Path, features: &str) -> Result<()> {
    let args = [
        OsStr::new("build"),
        OsStr::new("--release"),
        OsStr::new("--manifest-path"),
        manifest.as_os_str(),
        OsStr::new("--target-dir"),
        target.as_os_str(),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
    ];
    cmd::run_inherit(&cmd::cargo(), &args, root)
}

/// The crates that own a symbol of `bin`, as `nm` lists its symbol table.
fn owners(root: &Path, bin: &Path) -> Result<BTreeSet<String>> {
    let stdout = cmd::run_capture(OsStr::new("nm"), &[bin.as_os_str()], root, &[])?;
    let listing = String::from_utf8_lossy(&stdout);
    let mut owners = BTreeSet::new();
    for line in listing.lines() {
        // `nm` writes `ADDRESS TYPE NAME`, and leaves the address out of an
        // undefined symbol — which belongs to no object here.
        let mut fields = line.split_whitespace();
        let (Some(first), Some(second)) = (fields.next(), fields.next()) else {
            continue;
        };
        let name = match fields.next() {
            Some(name) => name,
            None if first == "U" || first == "w" || first == "v" => continue,
            None => second,
        };
        if let Some(owner) = owner(name) {
            owners.insert(owner.to_owned());
        }
        if name.contains(BUNDLED_TZDB_PATH) {
            owners.insert(BUNDLED_TZDB.to_owned());
        }
    }
    Ok(owners)
}

/// The crate a mangled symbol's path begins with: `jiff` for
/// `_RNvNtCs1_4jiff2tz2db` and for `_ZN4jiff2tz2db17h…E`, `core` for a
/// `core::ptr::drop_glue` instantiated in jiff's object. `None` when the
/// name is not a mangled Rust path.
///
/// In v0 a path is written outermost first, so the first crate root in the
/// name is the item's own; `C` introduces one, optionally with an
/// `s<base62>_` disambiguator, and the crate's name follows as a length and
/// the bytes. The legacy scheme writes the crate first after `_ZN`.
fn owner(symbol: &str) -> Option<&str> {
    if let Some(rest) = symbol.strip_prefix("_ZN") {
        return length_prefixed(rest);
    }
    let rest = symbol.strip_prefix("_R")?;
    let at = rest.find('C')?;
    let after = &rest[at + 1..];
    match after.strip_prefix('s') {
        Some(disambiguated) => {
            let end = disambiguated.find('_')?;
            length_prefixed(&disambiguated[end + 1..])
        }
        None => length_prefixed(after),
    }
}

/// The name a decimal length prefixes, at the start of `s`.
fn length_prefixed(s: &str) -> Option<&str> {
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let length: usize = s.get(..digits)?.parse().ok()?;
    s.get(digits..digits.checked_add(length)?)
}

/// The report, printed and written to `target/native-canaries/report.md`.
fn report(rows: &[Linked]) -> String {
    let mut s = String::from(
        "# Native canaries\n\n\
         Which crates a feature set links into a native binary, read from the\n\
         symbols of an unstripped release build of `tools/native-canary`\n\
         (`cargo xtask native-canaries`).\n\n\
         | feature set | binary |",
    );
    for crate_name in CRATES {
        let _ = write!(s, " `{crate_name}` |");
    }
    s.push_str("\n|---|---:|");
    for _ in CRATES {
        s.push_str("---|");
    }
    s.push('\n');
    for linked in rows {
        let _ = write!(s, "| `{}` | {} B |", linked.row.features, linked.size);
        for crate_name in CRATES {
            let mark = if linked.present.contains(crate_name) {
                "linked"
            } else {
                "absent"
            };
            let _ = write!(s, " {mark} |");
        }
        s.push('\n');
    }
    s.push('\n');
    for linked in rows {
        let _ = writeln!(s, "* `{}`: {}.", linked.row.features, linked.row.what);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::owner;

    /// A symbol belongs to the crate its path begins with, never to the one
    /// that instantiated it — the difference between a forbidding row that
    /// means something and one that can never hold.
    #[test]
    fn a_symbol_belongs_to_the_crate_its_path_begins_with() {
        assert_eq!(
            owner("_RINvMs_NtCs6trkG1KX5j1_4jiff5errorNtB5_5Error7contextE"),
            Some("jiff")
        );
        assert_eq!(
            owner(
                "_RINvNtCsgxBkk5gSRhY_4core3ptr9drop_glueNtNtCs1_5alloc6string6StringECs6trkG1KX5j1_4jiff.590"
            ),
            Some("core")
        );
        assert_eq!(owner("_ZN4jiff2tz2db17h0123456789abcdefE"), Some("jiff"));
        assert_eq!(
            owner("_RNvNtCs1_21unicode_normalization7recompose4next"),
            Some("unicode_normalization")
        );
        assert_eq!(owner("main"), None);
        assert_eq!(owner("_RNvC"), None);
    }

    /// The bundled database is jiff's own module, so it is recognised by
    /// that path and not by a crate name — including on an item of another
    /// crate instantiated for one of its types, which is just as good
    /// evidence that the bundle was linked.
    #[test]
    fn the_bundled_database_is_recognised_by_its_module_path() {
        for symbol in [
            "_RNvNtNtNtNtNtCs44JDraXbvz1_4jiff2tz2db7bundled5inner6global12CACHED_ZONES",
            "_ZN4jiff2tz2db7bundled5inner17h0123456789abcdefE",
            "_RINvNtCsgxBkk5gSRhY_4core3ptr9drop_glueNtNtNtNtNtNtCs44JDraXbvz1_4jiff2tz2db7bundled5inner6global10CachedZoneEBN_",
        ] {
            assert!(symbol.contains(super::BUNDLED_TZDB_PATH), "{symbol}");
        }
        assert!(!"_RNvNtNtCs44JDraXbvz1_4jiff2tz11TimeZone3new".contains(super::BUNDLED_TZDB_PATH));
    }
}
