//! `cargo xtask native-canaries`: what a feature set *links* into a native
//! binary (`plan/01` §6.1, guard rail 1).
//!
//! A size claim about a feature is only worth what the linker does with it.
//! For each row of [`ROWS`] this links `tools/native-canary` — the smallest
//! application that uses MF2: a typed language, a plain message, a number
//! and, with `fn-datetime`, a date in a named time zone — in the profile
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
//! There are no forbidding rows yet: today `mf2-host-std` depends on jiff,
//! `unicode-normalization` and `ryu` unconditionally, so every native binary
//! links them whether it formats a date or not. Phases 12 and 13 make those
//! dependencies conditional, and add the rows that hold them so.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The crates whose presence every row reports: the four heavy native
/// dependencies of `mf2-host-std` and `mf2`'s own `native` additions.
const CRATES: &[&str] = &["jiff", "unicode_normalization", "ryu", "sha2", "sys_locale"];

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
        forbids: &[],
    },
    Row {
        // The positive control: a date in a named zone reaches
        // `Host::zone_offset`, which is jiff. If this row reported no jiff,
        // the reader would be broken and every forbidding row vacuous.
        what: "a native application formatting a date in a named zone (the positive control)",
        features: "native,fn-datetime",
        requires: &["jiff"],
        forbids: &[],
    },
];

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
}
