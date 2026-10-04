//! `cargo xtask feature-costs`: what each function and data feature of `mf2`
//! adds to what ships (`plan/01` §6, guard rail 4).
//!
//! Every cost is a difference of two builds that differ by the one feature,
//! on a corpus that uses it:
//!
//! * **in the browser**, the gzip bytes of the reference workload's client
//!   wasm at the small scale, built and shipped exactly as `cargo xtask size`
//!   builds it ([`b5::build`]), from a workload generated with `:number`
//!   messages and, for the date features, `:datetime` messages too;
//! * **in a native binary**, the stripped bytes of `tools/native-canary`, the
//!   application `cargo xtask native-canaries` links, in its release profile.
//!
//! The table is written to [`TABLE`], which the user guide includes. With
//! `--check` (the nightly run) nothing is written there: each fresh figure is
//! held to the committed one, and the run fails when one is off by more than
//! [`off`] allows — the figures are the guide's, so they must stay true.
//!
//! `static-locale` and `mark-fallback-lang` change the Leptos layer, which
//! the workload does not have; the table says they are not measured here.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::b5;
use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The committed table, relative to the repository root.
const TABLE: &str = "docs/feature-costs.md";

/// `:number` messages in both client corpora, and `:datetime` messages in the
/// dates corpus (of 1,600 messages at the small scale).
const NUMBERS: &str = "150";
const DATES: &str = "150";

/// Where a cost is paid.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Client,
    Native,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Client => "browser",
            Side::Native => "native",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Side::Client => "B gzip",
            Side::Native => "B stripped",
        }
    }
}

/// The client's two corpora.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Corpus {
    /// The reference workload with `:number` messages.
    Numbers,
    /// The same with `:datetime` messages as well.
    Dates,
}

/// One build: a side, the client's corpus, and the features — the client
/// app's `--features` (always with `hydrate`; the functions through its i18n
/// crate, whose own features are what `mf2-build` reads), or the canary's.
#[derive(Clone, Copy)]
struct Build {
    side: Side,
    corpus: Corpus,
    features: &'static str,
}

const fn client(corpus: Corpus, features: &'static str) -> Build {
    Build {
        side: Side::Client,
        corpus,
        features,
    }
}

const fn native(features: &'static str) -> Build {
    Build {
        side: Side::Native,
        // The canary picks its corpus by its date formatter itself.
        corpus: Corpus::Numbers,
        features,
    }
}

/// One row: the feature, the build without it and the build with it, and
/// what the corpus is.
struct Cost {
    feature: &'static str,
    without: Build,
    with: Build,
    corpus: &'static str,
    /// The feature is on and nothing uses it, so the row should be 0.
    unused: bool,
}

const C_NUMBERS: Build = client(Corpus::Numbers, "hydrate");
const C_NUMBER: Build = client(Corpus::Numbers, "hydrate,workload-i18n/fn-number");
const C_NUMBER_INTL: Build = client(Corpus::Numbers, "hydrate,workload-i18n/number-intl");
const C_DATE: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/fn-number,workload-i18n/host-web-datetime-iso",
);
const C_DATE_ICU: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/fn-number,workload-i18n/host-web-datetime-icu",
);
const C_DATE_INTL: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/fn-number,workload-i18n/host-web-datetime-intl",
);
const N_BASE: Build = native("native");
const N_DATE: Build = native("native,native-datetime-iso");

const WORKLOAD_NUMBERS: &str = "the reference workload, 150 messages with `:number`";
const WORKLOAD_DATES: &str =
    "the reference workload, 150 messages with `:number` and 150 with `:datetime`";
const CANARY: &str = "a plain message and a plural";
const CANARY_DATES: &str = "a plain message, a plural and a date in a named zone";

/// Every row, in the order the table shows them.
const COSTS: &[Cost] = &[
    Cost {
        feature: "fn-number",
        without: C_NUMBERS,
        with: C_NUMBER,
        corpus: WORKLOAD_NUMBERS,
        unused: false,
    },
    Cost {
        feature: "number-intl",
        without: C_NUMBER,
        with: C_NUMBER_INTL,
        corpus: WORKLOAD_NUMBERS,
        unused: false,
    },
    Cost {
        // A build without a date formatter refuses a `:datetime` message, so the
        // build without it reads the numbers corpus.
        feature: "host-web-datetime-iso",
        without: C_NUMBER,
        with: C_DATE,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "host-web-datetime-icu",
        without: C_DATE,
        with: C_DATE_ICU,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "host-web-datetime-intl",
        without: C_DATE,
        with: C_DATE_INTL,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "fn-number",
        without: N_BASE,
        with: native("native,fn-number"),
        corpus: CANARY,
        unused: false,
    },
    Cost {
        feature: "native-datetime-iso",
        without: N_BASE,
        with: N_DATE,
        corpus: CANARY_DATES,
        unused: false,
    },
    Cost {
        feature: "tzdb-bundled",
        without: N_DATE,
        with: native("native,native-datetime-iso,tzdb-bundled"),
        corpus: CANARY_DATES,
        unused: false,
    },
    Cost {
        feature: "tzdb-bundled",
        without: N_BASE,
        with: native("native,tzdb-bundled"),
        corpus: "no date in any message, so the feature is on and unused",
        unused: true,
    },
    Cost {
        feature: "compile",
        without: N_BASE,
        with: native("native,compile"),
        corpus: "the canary's messages, and one compiled at run time",
        unused: false,
    },
    Cost {
        feature: "ratatui",
        without: native("native,tui"),
        with: native("ratatui"),
        corpus: "a message drawn as a Ratatui `Line`",
        unused: false,
    },
    Cost {
        feature: "clap",
        without: native("native,cli"),
        with: native("native,clap"),
        corpus: "`--lang` parsed by clap",
        unused: false,
    },
];

/// A fresh figure is off from the committed one when they differ by more
/// than a tenth of the committed figure and by more than 128 bytes: room for
/// a toolchain's drift, none for a feature that starts linking something.
fn off(committed: i64, fresh: i64) -> bool {
    let diff = (fresh - committed).abs();
    diff > committed.abs() / 10 && diff > 128
}

pub(crate) fn run(root: &Path, check: bool, keep: bool) -> Result<()> {
    let out = root.join("target/feature-costs");
    if !keep && out.exists() {
        fs::remove_dir_all(&out).map_err(|source| Error::IoAt {
            path: out.clone(),
            source,
        })?;
    }

    let mut sizes: BTreeMap<(bool, bool, &'static str), u64> = BTreeMap::new();
    let mut figures = Vec::with_capacity(COSTS.len());
    for cost in COSTS {
        let without = size(root, &out, cost.without, &mut sizes)?;
        let with = size(root, &out, cost.with, &mut sizes)?;
        let figure = i64::try_from(with).unwrap_or(i64::MAX) - i64::try_from(without).unwrap_or(0);
        eprintln!(
            "feature-costs: `{}` adds {figure} {} ({})",
            cost.feature,
            cost.with.side.unit(),
            cost.with.side.name()
        );
        figures.push(figure);
    }

    let table = table(&figures, &today(), &rustc(root)?);
    fsx::write(&out.join("feature-costs.md"), table.as_bytes())?;
    print!("{table}");

    for (cost, figure) in COSTS.iter().zip(&figures) {
        if cost.unused && off(0, *figure) {
            eprintln!(
                "feature-costs: finding: `{}` is on and unused and still adds {figure} {}",
                cost.feature,
                cost.with.side.unit()
            );
        }
    }

    if !check {
        fsx::write(&root.join(TABLE), table.as_bytes())?;
        eprintln!("feature-costs: wrote {TABLE}");
        return Ok(());
    }
    let committed = committed(&fsx::read_to_string(&root.join(TABLE))?);
    let mut failures = Vec::new();
    for (cost, figure) in COSTS.iter().zip(&figures) {
        let key = key(cost);
        match committed.get(&key) {
            Some(old) if off(*old, *figure) => {
                failures.push(format!("{key}: {figure} measured, {old} committed"));
            }
            Some(_) => {}
            None => failures.push(format!("{key}: not in {TABLE}")),
        }
    }
    if failures.is_empty() {
        eprintln!("feature-costs: every figure of {TABLE} holds");
        Ok(())
    } else {
        Err(Error::FeatureCosts(format!(
            "{} (rerun `cargo xtask feature-costs` and commit {TABLE})",
            failures.join("; ")
        )))
    }
}

/// One build's size, built once however many rows read it.
fn size(
    root: &Path,
    out: &Path,
    build: Build,
    sizes: &mut BTreeMap<(bool, bool, &'static str), u64>,
) -> Result<u64> {
    let key = (
        build.side == Side::Client,
        build.corpus == Corpus::Dates,
        build.features,
    );
    if let Some(size) = sizes.get(&key) {
        return Ok(*size);
    }
    let size = match build.side {
        Side::Client => client_size(root, out, build)?,
        Side::Native => native_size(root, out, build.features)?,
    };
    sizes.insert(key, size);
    Ok(size)
}

/// The client wasm of the workload's `tr` app, shipped and gzipped.
fn client_size(root: &Path, out: &Path, build: Build) -> Result<u64> {
    let (name, extra): (&str, &[&str]) = match build.corpus {
        Corpus::Numbers => ("wl-numbers", &["--number", NUMBERS]),
        Corpus::Dates => ("wl-dates", &["--number", NUMBERS, "--datetime", DATES]),
    };
    let workload = std::path::absolute(out.join(name)).map_err(|source| Error::IoAt {
        path: out.join(name),
        source,
    })?;
    let [scale, _] = &b5::SCALES;
    b5::generate(root, &workload, scale, b5::Mode::String, extra)?;
    eprintln!(
        "feature-costs: building the client with `{}`",
        build.features
    );
    Ok(b5::build(root, &workload, "tr", build.features)?.opt_gz)
}

/// The canary application, linked with `features` and stripped.
fn native_size(root: &Path, out: &Path, features: &str) -> Result<u64> {
    let target = out.join("native");
    eprintln!("feature-costs: linking the canary with `{features}`");
    let manifest = root.join("tools/native-canary/Cargo.toml");
    cmd::run_inherit_env(
        &cmd::cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("--release"),
            OsStr::new("--manifest-path"),
            manifest.as_os_str(),
            OsStr::new("--target-dir"),
            target.as_os_str(),
            OsStr::new("--no-default-features"),
            OsStr::new("--features"),
            OsStr::new(features),
        ],
        root,
        &[("CARGO_PROFILE_RELEASE_STRIP", OsStr::new("symbols"))],
    )?;
    let bin = target.join("release/native-canary");
    fs::metadata(&bin)
        .map(|m| m.len())
        .map_err(|source| Error::IoAt { path: bin, source })
}

/// How a row is known in the table: feature, side and what it is set against.
fn key(cost: &Cost) -> String {
    format!(
        "`{}` | {} | {}",
        cost.feature,
        cost.with.side.name(),
        shown(cost.without)
    )
}

/// A build's features as the table shows them: `mf2`'s names, without the
/// client's `hydrate` and the crate prefixes.
fn shown(build: Build) -> String {
    let names: Vec<String> = build
        .features
        .split(',')
        .filter(|f| *f != "hydrate")
        .map(|f| {
            let name = f.rsplit('/').next().unwrap_or(f);
            format!("`{name}`")
        })
        .collect();
    if names.is_empty() {
        "no function".to_owned()
    } else {
        names.join(", ")
    }
}

/// The table, as the guide includes it.
fn table(figures: &[i64], date: &str, rustc: &str) -> String {
    let mut s = String::from(
        "<!-- Written by `cargo xtask feature-costs`, and held to a fresh \
         measurement every night: do not edit by hand. -->\n\n\
         | Feature | Where | Set against | Adds | Measured on |\n\
         |---|---|---|---:|---|\n",
    );
    for (cost, figure) in COSTS.iter().zip(figures) {
        let _ = writeln!(
            s,
            "| {} | {} {} | {} |",
            key(cost),
            thousands(*figure),
            cost.with.side.unit(),
            cost.corpus
        );
    }
    let _ = write!(
        s,
        "\nA figure is the size with the feature less the size without it. \
         In the browser: the client wasm of the reference workload at 1,860 \
         call sites, built for `hydrate`, through `wasm-bindgen` and \
         `wasm-opt -Oz`, then `gzip -9`. Native: the smallest native MF2 \
         application (`tools/native-canary`), in release with fat LTO, \
         stripped. `static-locale` and `mark-fallback-lang` change the Leptos \
         layer, which the reference workload does not have, so they are not \
         measured here.\n\n\
         Measured on {date} by `cargo xtask feature-costs`, with {rustc}.\n"
    );
    s
}

/// The committed figures, by row key.
fn committed(text: &str) -> BTreeMap<String, i64> {
    let mut figures = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // `| feature | where | against | adds | corpus |` splits into seven.
        let [_, feature, side, against, adds, _, _] = cells.as_slice() else {
            continue;
        };
        if !feature.starts_with('`') {
            continue;
        }
        let digits: String = adds
            .split(' ')
            .next()
            .unwrap_or("")
            .chars()
            .filter(|c| *c != ',')
            .collect();
        if let Ok(figure) = digits.parse() {
            figures.insert(format!("{feature} | {side} | {against}"), figure);
        }
    }
    figures
}

/// `12345` as `12,345`.
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut s = String::new();
    if n < 0 {
        s.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            s.push(',');
        }
        s.push(c);
    }
    s
}

/// Today's date in UTC, as `YYYY-MM-DD`.
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    // Howard Hinnant's `civil_from_days`.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The compiler the figures were measured with, as `rustc --version` says.
fn rustc(root: &Path) -> Result<String> {
    let stdout = cmd::run_capture(OsStr::new("rustc"), &[OsStr::new("--version")], root, &[])?;
    let version = String::from_utf8_lossy(&stdout);
    let version = version.split(" (").next().unwrap_or("rustc").trim();
    Ok(format!("`{version}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the table writes, `--check` reads back: every row, by its key.
    #[test]
    fn the_table_reads_back() {
        let figures: Vec<i64> = (0..COSTS.len())
            .map(|i| i64::try_from(i).unwrap_or(0) * 1_234 - 600)
            .collect();
        let read = committed(&table(&figures, "2026-10-03", "`rustc 1.0.0`"));
        assert_eq!(read.len(), COSTS.len());
        for (cost, figure) in COSTS.iter().zip(&figures) {
            assert_eq!(read.get(&key(cost)), Some(figure), "{}", key(cost));
        }
    }

    #[test]
    fn tolerance_and_dates() {
        assert!(!off(10_000, 10_900));
        assert!(off(10_000, 11_100));
        assert!(!off(0, 128));
        assert!(off(0, 129));
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(-1_000), "-1,000");
        assert_eq!(thousands(999), "999");
        assert_eq!(today().len(), 10);
    }
}
