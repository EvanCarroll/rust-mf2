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
//! The date features are measured under the family names an application
//! turns on (`plan/08` §7): `leptos-client-datetime-*` in the browser,
//! `native-datetime-*` natively. A date feature that is on in an application
//! with no date and plain placeholders has a row of its own on each side, so
//! what it costs when nothing shows a date stays visible. A second table
//! gives, per language, the brotli bytes the date slice adds to a browser
//! catalog with `icu`, and with `intl` (which should be 0).
//!
//! The tables are written to [`TABLE`], which the user guide includes. With
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
    /// The feature is on and nothing uses it: the smallest build would add
    /// 0, so a run reports anything more as a finding.
    unused: bool,
}

// A message that formats a number builds only where its side has named a
// number formatter, so every build names one: `plain`, the smallest that
// builds, is what the other two are set against, and the date rows keep
// `builtin` on throughout.
const C_PLAIN: Build = client(
    Corpus::Numbers,
    "hydrate,workload-i18n/leptos-client-number-plain",
);
const C_BUILTIN: Build = client(
    Corpus::Numbers,
    "hydrate,workload-i18n/leptos-client-number-builtin",
);
const C_NUMBER_INTL: Build = client(
    Corpus::Numbers,
    "hydrate,workload-i18n/leptos-client-number-intl",
);
const C_DATE_ISO: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/leptos-client-number-builtin,workload-i18n/leptos-client-datetime-iso",
);
const C_DATE_ICU: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/leptos-client-number-builtin,workload-i18n/leptos-client-datetime-icu",
);
const C_DATE_INTL: Build = client(
    Corpus::Dates,
    "hydrate,workload-i18n/leptos-client-number-builtin,workload-i18n/leptos-client-datetime-intl",
);
/// `intl` on, over the corpus with no date: what the feature costs when
/// nothing shows a date. It had never been measured.
const C_INTL_NO_DATE: Build = client(
    Corpus::Numbers,
    "hydrate,workload-i18n/leptos-client-number-builtin,workload-i18n/leptos-client-datetime-intl",
);
const N_BASE: Build = native("native,native-number-plain");
const N_ISO: Build = native("native,native-number-plain,native-datetime-iso");
const N_ICU: Build = native("native,native-number-plain,native-datetime-icu");
/// `icu` on, and the canary keeps its corpus with no date
/// (`no-date-message`, a feature of the canary alone).
const N_ICU_NO_DATE: Build =
    native("native,native-number-plain,native-datetime-icu,no-date-message");

const WORKLOAD_NUMBERS: &str = "the reference workload, 150 messages with `:number`";
const WORKLOAD_DATES: &str =
    "the reference workload, 150 messages with `:number` and 150 with `:datetime`";
const WORKLOAD_NO_DATE: &str = "the reference workload, 150 messages with `:number` and plain \
     placeholders, no date: the feature is on and nothing shows a date";
const CANARY: &str = "a plain message, a plain placeholder and a plural";
const CANARY_DATES: &str =
    "a plain message, a plain placeholder, a plural and a date in a named zone";
const CANARY_NO_DATE: &str = "a plain message, a plain placeholder and a plural, no date: the \
     feature is on and nothing shows a date";

/// Every row, in the order the table shows them.
const COSTS: &[Cost] = &[
    Cost {
        feature: "leptos-client-number-builtin",
        without: C_PLAIN,
        with: C_BUILTIN,
        corpus: WORKLOAD_NUMBERS,
        unused: false,
    },
    Cost {
        feature: "leptos-client-number-intl",
        without: C_PLAIN,
        with: C_NUMBER_INTL,
        corpus: WORKLOAD_NUMBERS,
        unused: false,
    },
    Cost {
        // A build without a date formatter refuses a `:datetime` message, so the
        // build without it reads the numbers corpus.
        feature: "leptos-client-datetime-iso",
        without: C_BUILTIN,
        with: C_DATE_ISO,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "leptos-client-datetime-icu",
        without: C_DATE_ISO,
        with: C_DATE_ICU,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "leptos-client-datetime-intl",
        without: C_DATE_ISO,
        with: C_DATE_INTL,
        corpus: WORKLOAD_DATES,
        unused: false,
    },
    Cost {
        feature: "leptos-client-datetime-intl",
        without: C_BUILTIN,
        with: C_INTL_NO_DATE,
        corpus: WORKLOAD_NO_DATE,
        unused: true,
    },
    Cost {
        feature: "native-number-builtin",
        without: N_BASE,
        with: native("native,native-number-builtin"),
        corpus: CANARY,
        unused: false,
    },
    Cost {
        feature: "native-datetime-iso",
        without: N_BASE,
        with: N_ISO,
        corpus: CANARY_DATES,
        unused: false,
    },
    Cost {
        feature: "native-datetime-icu",
        without: N_BASE,
        with: N_ICU,
        corpus: CANARY_DATES,
        unused: false,
    },
    Cost {
        feature: "native-datetime-icu",
        without: N_BASE,
        with: N_ICU_NO_DATE,
        corpus: CANARY_NO_DATE,
        unused: true,
    },
    Cost {
        feature: "tzdb-bundled",
        without: N_ISO,
        with: native("native,native-number-plain,native-datetime-iso,tzdb-bundled"),
        corpus: CANARY_DATES,
        unused: false,
    },
    Cost {
        feature: "tzdb-bundled",
        without: N_BASE,
        with: native("native,native-number-plain,tzdb-bundled"),
        corpus: "no date in any message, so the feature is on and unused",
        unused: true,
    },
    Cost {
        feature: "compile",
        without: N_BASE,
        with: native("native,native-number-plain,compile"),
        corpus: "the canary's messages, and one compiled at run time",
        unused: false,
    },
    Cost {
        feature: "ratatui",
        without: native("native,native-number-plain,tui"),
        with: native("ratatui,native-number-plain"),
        corpus: "a message drawn as a Ratatui `Line`",
        unused: false,
    },
    Cost {
        feature: "clap",
        without: native("native,native-number-plain,cli"),
        with: native("native,native-number-plain,clap"),
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
    let slices = slices(root, &out)?;
    for slice in &slices {
        eprintln!(
            "feature-costs: the date slice adds {} B brotli to `{}`'s catalog with `icu`, {} with `intl`",
            slice.icu, slice.tag, slice.intl
        );
    }

    let table = table(&figures, &slices, &today(), &rustc(root)?);
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
    for slice in &slices {
        if off(0, slice.intl) {
            eprintln!(
                "feature-costs: finding: with `intl` the date slice still adds {} B brotli to \
                 `{}`'s catalog, which no side of that build reads",
                slice.intl, slice.tag
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
    let fresh = COSTS
        .iter()
        .zip(&figures)
        .map(|(cost, figure)| (id(cost), *figure))
        .chain(slices.iter().flat_map(|slice| {
            [
                (slice_id(&slice.tag, "icu"), slice.icu),
                (slice_id(&slice.tag, "intl"), slice.intl),
            ]
        }));
    for (id, figure) in fresh {
        match committed.get(&id) {
            Some(old) if off(*old, figure) => {
                failures.push(format!("{id}: {figure} measured, {old} committed"));
            }
            Some(_) => {}
            None => failures.push(format!("{id}: not in {TABLE}")),
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

/// The reference workload of `corpus` at the small scale, generated under
/// `out`.
fn workload(root: &Path, out: &Path, corpus: Corpus) -> Result<std::path::PathBuf> {
    let (name, extra): (&str, &[&str]) = match corpus {
        Corpus::Numbers => ("wl-numbers", &["--number", NUMBERS]),
        Corpus::Dates => ("wl-dates", &["--number", NUMBERS, "--datetime", DATES]),
    };
    let workload = std::path::absolute(out.join(name)).map_err(|source| Error::IoAt {
        path: out.join(name),
        source,
    })?;
    let [scale, _] = &b5::SCALES;
    b5::generate(root, &workload, scale, b5::Mode::String, extra)?;
    Ok(workload)
}

/// The client wasm of the workload's `tr` app, shipped and gzipped.
fn client_size(root: &Path, out: &Path, build: Build) -> Result<u64> {
    let workload = workload(root, out, build.corpus)?;
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

/// One language's date slice in a browser catalog: the brotli bytes it adds
/// with `icu`, and with `intl`, where no side reads it and it should be 0.
struct SliceCost {
    tag: String,
    icu: i64,
    intl: i64,
}

/// The brotli bytes the date slice adds to each language's browser catalog:
/// the dates corpus's catalogs with `icu` and with `intl`, less the same with
/// `iso`, which has no slice. `mf2-build` builds them from the features the
/// workload's i18n crate sees in each client build, and writes nothing.
fn slices(root: &Path, out: &Path) -> Result<Vec<SliceCost>> {
    let workload = workload(root, out, Corpus::Dates)?;
    let iso = catalogs(&workload, out, C_DATE_ISO)?;
    let icu = catalogs(&workload, out, C_DATE_ICU)?;
    let intl = catalogs(&workload, out, C_DATE_INTL)?;
    let added = |with: &BTreeMap<String, i64>, tag: &str, base: i64| {
        with.get(tag).map_or(0, |bytes| bytes - base)
    };
    Ok(iso
        .iter()
        .map(|(tag, base)| SliceCost {
            tag: tag.clone(),
            icu: added(&icu, tag, *base),
            intl: added(&intl, tag, *base),
        })
        .collect())
}

/// Each language's catalog as a browser downloads it, brotli bytes by tag:
/// the workload's corpus under `build`'s features, configured as the
/// workload's i18n crate configures its build (`bench/workload-gen`).
fn catalogs(workload: &Path, out: &Path, build: Build) -> Result<BTreeMap<String, i64>> {
    let mut config = mf2_build::Config::default();
    "en".clone_into(&mut config.source_locale);
    for &lint in mf2_build::Lint::ALL {
        let floor = lint.floor();
        if floor != mf2_build::Level::Error {
            config.lints.insert(lint, floor);
        }
    }
    eprintln!(
        "feature-costs: building the catalogs with `{}`",
        build.features
    );
    let outcome = mf2_build::Build::at(workload, out.join("slice-catalogs"))
        .config(config)
        .features(mf2_build::Features::parse(build.features))
        .check()?
        .into_result()?;
    Ok(outcome
        .catalogs
        .iter()
        .map(|catalog| {
            (
                catalog.tag.clone(),
                i64::try_from(catalog.br.len()).unwrap_or(i64::MAX),
            )
        })
        .collect())
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

/// How `--check` knows a row: its key and its corpus, since one feature can
/// be set against the same build over two corpora.
fn id(cost: &Cost) -> String {
    format!("{} | {}", key(cost), cost.corpus)
}

/// How `--check` knows one language's slice figure for one formatter.
fn slice_id(tag: &str, formatter: &str) -> String {
    format!("slice `{tag}` | {formatter}")
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

/// The tables, as the guide includes them.
fn table(figures: &[i64], slices: &[SliceCost], date: &str, rustc: &str) -> String {
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
    s.push_str(
        "\n**The date slice, per language.** The brotli bytes the date data adds \
         to each language's catalog, which a browser downloads: the reference \
         workload with `:datetime` messages, built for `hydrate` with each \
         browser formatter, less the same with `leptos-client-datetime-iso`. \
         With `intl` the browser formats dates itself and no date data is \
         downloaded, so that column should be 0.\n\n\
         | Language | With `leptos-client-datetime-icu` | With `leptos-client-datetime-intl` |\n\
         |---|---:|---:|\n",
    );
    for slice in slices {
        let _ = writeln!(
            s,
            "| `{}` | {} B brotli | {} B brotli |",
            slice.tag,
            thousands(slice.icu),
            thousands(slice.intl)
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

/// The committed figures, by [`id`] and [`slice_id`].
fn committed(text: &str) -> BTreeMap<String, i64> {
    let mut figures = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        match cells.as_slice() {
            // `| feature | where | against | adds | corpus |` splits into seven.
            [_, feature, side, against, adds, corpus, _] if feature.starts_with('`') => {
                if let Some(figure) = figure(adds) {
                    figures.insert(format!("{feature} | {side} | {against} | {corpus}"), figure);
                }
            }
            // `| language | icu | intl |` into five.
            [_, tag, icu, intl, _] if tag.starts_with('`') => {
                let tag = tag.trim_matches('`');
                for (formatter, cell) in [("icu", icu), ("intl", intl)] {
                    if let Some(figure) = figure(cell) {
                        figures.insert(slice_id(tag, formatter), figure);
                    }
                }
            }
            _ => {}
        }
    }
    figures
}

/// A cell's figure: `12,345 B gzip` as `12345`.
fn figure(cell: &str) -> Option<i64> {
    let digits: String = cell
        .split(' ')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| *c != ',')
        .collect();
    digits.parse().ok()
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

    /// What the tables write, `--check` reads back: every row, by its id,
    /// and every language's two slice figures. No two rows share an id.
    #[test]
    fn the_table_reads_back() {
        let figures: Vec<i64> = (0..COSTS.len())
            .map(|i| i64::try_from(i).unwrap_or(0) * 1_234 - 600)
            .collect();
        let slices = [
            SliceCost {
                tag: "ar".to_owned(),
                icu: 1_435,
                intl: 0,
            },
            SliceCost {
                tag: "en-XA".to_owned(),
                icu: 349,
                intl: -3,
            },
        ];
        let page = table(&figures, &slices, "2026-10-03", "`rustc 1.0.0`");
        let read = committed(&page);
        assert_eq!(read.len(), COSTS.len() + 2 * slices.len());
        for (cost, figure) in COSTS.iter().zip(&figures) {
            assert_eq!(read.get(&id(cost)), Some(figure), "{}", id(cost));
        }
        for slice in &slices {
            assert_eq!(read.get(&slice_id(&slice.tag, "icu")), Some(&slice.icu));
            assert_eq!(read.get(&slice_id(&slice.tag, "intl")), Some(&slice.intl));
        }
    }

    /// The rows `plan/08` §7 asks for: the browser's date families by name,
    /// both native formatters against no date feature, and a date feature
    /// that is on with nothing to show on each side.
    #[test]
    fn the_date_rows_are_there() {
        let row = |feature: &str, side: Side, unused: bool| {
            COSTS
                .iter()
                .any(|c| c.feature == feature && c.with.side == side && c.unused == unused)
        };
        for formatter in ["iso", "icu", "intl"] {
            let feature = format!("leptos-client-datetime-{formatter}");
            assert!(row(&feature, Side::Client, false), "{feature}");
        }
        assert!(row("leptos-client-datetime-intl", Side::Client, true));
        assert!(row("native-datetime-iso", Side::Native, false));
        assert!(row("native-datetime-icu", Side::Native, false));
        assert!(row("native-datetime-icu", Side::Native, true));
        assert!(!COSTS.iter().any(|c| c.feature.starts_with("host-web-")));
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
