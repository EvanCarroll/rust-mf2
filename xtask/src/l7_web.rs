//! `cargo xtask l7-web`: conformance layer L7 in the browser.
//!
//! L7 is L6(b)'s page and twin switch delivered two other ways — as islands
//! (columns `L7`, `L7d`) and client-only (`L7c`, `L7cd`) — and only an engine
//! can run it, so this command is its harness, as `cargo xtask l4-web` is
//! for the `intl` entries:
//!
//! 1. for each configuration (`all`: every function feature; `default`: none),
//!    `conformance/l7-web`'s `l7-page` binary renders each of the four sets'
//!    pages into `target/l7-web/<configuration>/`, and the same crate is built
//!    for `wasm32-unknown-unknown` twice — `hydrate` (islands) and `csr` —
//!    and bound with `wasm-bindgen`;
//! 2. `tools/e2e/checks/l7.mjs` drives every page in each engine and records,
//!    per case, whether the browser agreed with the server at every step;
//! 3. every runtime-valid test's four cells are judged from that record and
//!    from L6's own verdict on the test — a delivery mode cannot be greener
//!    than the render it delivers — and the ledger is held to them
//!    ([`mf2_conformance::verify`]); `--promote` tightens it instead
//!    ([`mf2_conformance::promote`]) and rewrites the report.
//!
//! Beside L7 it runs the date-formatter pages (`plan/08` §4.3): the en-US
//! set's islands page rendered by a server with the ISO stand-in
//! (`target/l7-web/dates-iso/`) and with ICU4X (`dates-icu/`), both
//! hydrated by a client with `Intl`; `tools/e2e/checks/dates-formatter.mjs`
//! checks that the first page's dates are rewritten after hydration and the
//! second's are not.
//!
//! In the default configuration a test whose message the build refuses (L6d
//! `build-reject`) is not on the page — no application in that
//! configuration could ship it — and its cell is `degraded` the same way;
//! a test on the page must agree in every engine.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use mf2_conformance::l4::DefaultOutcome;
use mf2_conformance::ledger::DegradedKind;
use mf2_conformance::{Column, LEDGER_PATH, Ledger, Results, TestKind};
use serde_json::Value;

use crate::cmd;
use crate::error::{Error, Result};
use crate::report;

/// The engines `all` means. Two are required (master plan §9); `WebKit`
/// joins wherever a build of it is installed.
pub(crate) const ENGINES: [&str; 3] = ["chromium", "firefox", "webkit"];

/// The four sets: every locale the suite's tests use.
const SETS: [&str; 4] = ["en-US", "und", "fr", "ar"];

/// A configuration: its directory under `target/l7-web/`, and whether every
/// function feature is on.
const CONFIGURATIONS: [(&str, bool); 2] = [("all", true), ("default", false)];

/// The date-formatter configurations: each one's directory under
/// `target/l7-web/` and its feature of `conformance/l7-web`.
const DATE_CONFIGURATIONS: [&str; 2] = ["dates-iso", "dates-icu"];

/// The set the date-formatter pages are rendered from.
const DATE_SET: &str = "en-US";

/// The two delivery modes: the check's name for each, and the columns it
/// judges in each configuration.
const MODES: [(&str, Column, Column); 2] = [
    ("islands", Column::L7, Column::L7d),
    ("csr", Column::L7c, Column::L7cd),
];

pub(crate) fn run(root: &Path, engines: &str, build: bool, promote: bool) -> Result<()> {
    let out = root.join("target/l7-web");
    if build {
        for (name, all) in CONFIGURATIONS {
            build_configuration(root, &out.join(name), all)?;
        }
        for name in DATE_CONFIGURATIONS {
            build_dates(root, &out.join(name), name)?;
        }
    }
    let report_path = out.join("report.json");
    eprintln!("l7-web: running tools/e2e/checks/l7.mjs in {engines}");
    let browser_ok = cmd::run_inherit(
        OsStr::new("node"),
        &[
            OsStr::new("run.mjs"),
            OsStr::new("l7"),
            OsStr::new("--browser"),
            OsStr::new(engines),
            OsStr::new("--json"),
            report_path.as_os_str(),
        ],
        &root.join("tools/e2e"),
    )
    .is_ok();

    let dates_report = out.join("dates-report.json");
    eprintln!("l7-web: running tools/e2e/checks/dates-formatter.mjs in {engines}");
    let dates_ok = cmd::run_inherit(
        OsStr::new("node"),
        &[
            OsStr::new("run.mjs"),
            OsStr::new("dates-formatter"),
            OsStr::new("--browser"),
            OsStr::new(engines),
            OsStr::new("--json"),
            dates_report.as_os_str(),
        ],
        &root.join("tools/e2e"),
    )
    .is_ok();

    let suite = mf2_conformance::load_suite(root)?;
    let record = Record::load(&out, &report_path)?;
    let results = judge(&suite, &record);
    for column in [Column::L7, Column::L7c, Column::L7d, Column::L7cd] {
        let (passed, run) = results.tally(column);
        eprintln!(
            "l7-web: {column} {passed}/{run} (+{} documented degradations) in {}",
            results.degradations(column),
            record.engines.join(", ")
        );
    }

    let ledger_path = root.join(LEDGER_PATH);
    let text = fs::read_to_string(&ledger_path).map_err(|source| Error::IoAt {
        path: ledger_path.clone(),
        source,
    })?;
    let mut ledger = Ledger::parse(&text)?;
    if promote {
        let changed = mf2_conformance::promote(&mut ledger, &results);
        fs::write(&ledger_path, ledger.to_toml()).map_err(|source| Error::IoAt {
            path: ledger_path.clone(),
            source,
        })?;
        eprintln!(
            "l7-web: promoted {changed} cell(s) in {}",
            ledger_path.display()
        );
        report::check(root, None, None)?;
    }
    let violations = mf2_conformance::verify(&ledger, &results);
    for v in violations.iter().take(20) {
        eprintln!("l7-web: {v}");
    }
    if !violations.is_empty() {
        return Err(Error::L7(format!(
            "{} ledger violation(s) in the L7 columns",
            violations.len()
        )));
    }
    let required = ["chromium", "firefox"];
    if let Some(missing) = required
        .iter()
        .find(|e| !record.engines.iter().any(|r| r == *e))
    {
        return Err(Error::L7(format!(
            "{missing} did not run; L7 is judged in two engines at least"
        )));
    }
    if !browser_ok {
        return Err(Error::L7(
            "the browser run failed (target/l7-web/report.json)".to_owned(),
        ));
    }
    if !dates_ok {
        return Err(Error::L7(
            "the date-formatter pages failed (target/l7-web/dates-report.json)".to_owned(),
        ));
    }
    eprintln!(
        "l7-web: the ledger's L7 columns hold in {}",
        record.engines.join(", ")
    );
    Ok(())
}

/// Renders the pages of one configuration and builds its two clients.
///
/// Three target directories, one per build kind, shared by both
/// configurations: cargo keeps each feature set's artifacts apart, and the
/// `l7-page` binary and the wasm are used as soon as they are built.
fn build_configuration(root: &Path, dir: &Path, all: bool) -> Result<()> {
    if dir.exists() {
        // Stale catalogs and pages from an earlier corpus would be served.
        fs::remove_dir_all(dir).map_err(|source| Error::IoAt {
            path: dir.to_owned(),
            source,
        })?;
    }
    let features = |kind: &str| {
        if all {
            format!("{kind},all-functions")
        } else {
            kind.to_owned()
        }
    };
    eprintln!("l7-web: rendering the pages ({}, ssr)", dir.display());
    let ssr = features("ssr");
    cargo(
        root,
        &["build", "--release", "--features", &ssr, "--bin", "l7-page"],
        "ssr",
    )?;
    let page = root.join("target/l7-web/cargo-ssr/release/l7-page");
    for set in SETS {
        cmd::run_inherit(page.as_os_str(), &[OsStr::new(set), dir.as_os_str()], root)?;
    }
    for (kind, pkg) in [("hydrate", "pkg-islands"), ("csr", "pkg-csr")] {
        eprintln!("l7-web: building the {kind} client ({})", dir.display());
        let features = features(kind);
        cargo(
            root,
            &[
                "build",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                &features,
                "--lib",
            ],
            kind,
        )?;
        let wasm = root.join(format!(
            "target/l7-web/cargo-{kind}/wasm32-unknown-unknown/release/mf2_l7_web.wasm"
        ));
        let pkg = dir.join(pkg);
        cmd::run_inherit(
            OsStr::new("wasm-bindgen"),
            &[
                OsStr::new("--target"),
                OsStr::new("web"),
                OsStr::new("--out-dir"),
                pkg.as_os_str(),
                wasm.as_os_str(),
            ],
            root,
        )?;
    }
    Ok(())
}

/// Renders the date-formatter page of one configuration (`feature`, a
/// feature of `conformance/l7-web`) and builds its islands client: one set,
/// one delivery mode, since only the formatter differs from L7's pages.
fn build_dates(root: &Path, dir: &Path, feature: &str) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|source| Error::IoAt {
            path: dir.to_owned(),
            source,
        })?;
    }
    eprintln!(
        "l7-web: rendering the date-formatter page ({}, ssr)",
        dir.display()
    );
    let ssr = format!("ssr,{feature}");
    cargo(
        root,
        &["build", "--release", "--features", &ssr, "--bin", "l7-page"],
        "ssr",
    )?;
    let page = root.join("target/l7-web/cargo-ssr/release/l7-page");
    cmd::run_inherit(
        page.as_os_str(),
        &[OsStr::new(DATE_SET), dir.as_os_str()],
        root,
    )?;
    eprintln!(
        "l7-web: building the date-formatter client ({})",
        dir.display()
    );
    let hydrate = format!("hydrate,{feature}");
    cargo(
        root,
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--features",
            &hydrate,
            "--lib",
        ],
        "hydrate",
    )?;
    let wasm =
        root.join("target/l7-web/cargo-hydrate/wasm32-unknown-unknown/release/mf2_l7_web.wasm");
    let pkg = dir.join("pkg-islands");
    cmd::run_inherit(
        OsStr::new("wasm-bindgen"),
        &[
            OsStr::new("--target"),
            OsStr::new("web"),
            OsStr::new("--out-dir"),
            pkg.as_os_str(),
            wasm.as_os_str(),
        ],
        root,
    )
}

/// `cargo <args>` on `conformance/l7-web`, in the target directory of `kind`.
fn cargo(root: &Path, args: &[&str], kind: &str) -> Result<()> {
    let target = format!("target/l7-web/cargo-{kind}");
    let mut all: Vec<&str> = args.to_vec();
    all.extend([
        "--manifest-path",
        "conformance/l7-web/Cargo.toml",
        "--target-dir",
        &target,
        "--no-default-features",
    ]);
    let all: Vec<&OsStr> = all.iter().map(OsStr::new).collect();
    cmd::run_inherit(&cmd::cargo(), &all, root)
}

/// What the pages hold and what the engines made of them.
struct Record {
    /// `(configuration, locale)` → the ids on that set's pages.
    ids: BTreeMap<(String, String), Vec<String>>,
    /// `(engine, configuration, mode, locale)` → the page's run.
    pages: BTreeMap<(String, String, String, String), Page>,
    /// The engines that ran.
    engines: Vec<String>,
}

/// One page in one engine.
struct Page {
    /// What went wrong with the page as a whole: every case fails with it.
    errors: Vec<String>,
    /// Case id → what went wrong with it.
    failures: BTreeMap<String, String>,
}

impl Record {
    fn load(out: &Path, report: &Path) -> Result<Self> {
        let mut ids = BTreeMap::new();
        for (configuration, _) in CONFIGURATIONS {
            for set in SETS {
                let path = out.join(configuration).join(format!("{set}.json"));
                let Ok(json) = read_json(&path) else { continue };
                let list = json["cases"]
                    .as_array()
                    .map(|cases| {
                        cases
                            .iter()
                            .filter_map(|c| c["id"].as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                ids.insert((configuration.to_owned(), set.to_owned()), list);
            }
        }
        let json = read_json(report)?;
        let mut pages = BTreeMap::new();
        let mut engines = Vec::new();
        for run in json["runs"].as_array().into_iter().flatten() {
            let engine = run["browser"].as_str().unwrap_or_default().to_owned();
            engines.push(engine.clone());
            for page in run["data"]["pages"].as_array().into_iter().flatten() {
                let text = |key: &str| page[key].as_str().unwrap_or_default().to_owned();
                let errors = page["errors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.as_str().map(str::to_owned))
                    .collect();
                let failures = page["failures"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(id, what)| (id.clone(), what.as_str().unwrap_or_default().to_owned()))
                    .collect();
                pages.insert(
                    (
                        engine.clone(),
                        text("configuration"),
                        text("mode"),
                        text("locale"),
                    ),
                    Page { errors, failures },
                );
            }
        }
        Ok(Record {
            ids,
            pages,
            engines,
        })
    }

    /// Whether the case `id` of `locale`'s page is on it in `configuration`.
    fn on_page(&self, configuration: &str, locale: &str, id: &str) -> bool {
        self.ids
            .get(&(configuration.to_owned(), locale.to_owned()))
            .is_some_and(|ids| ids.iter().any(|i| i == id))
    }

    /// Whether every engine agreed with the server on case `id`.
    fn agreed(
        &self,
        configuration: &str,
        mode: &str,
        locale: &str,
        id: &str,
    ) -> Result<(), String> {
        if self.engines.is_empty() {
            return Err("no engine ran".to_owned());
        }
        for engine in &self.engines {
            let key = (
                engine.clone(),
                configuration.to_owned(),
                mode.to_owned(),
                locale.to_owned(),
            );
            let Some(page) = self.pages.get(&key) else {
                return Err(format!("{engine}: the {mode} page of {locale} did not run"));
            };
            if let Some(error) = page.errors.first() {
                return Err(format!("{engine}: {mode} page: {error}"));
            }
            if let Some(what) = page.failures.get(id) {
                return Err(format!("{engine}: {what}"));
            }
        }
        Ok(())
    }
}

fn read_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(|source| Error::IoAt {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|e| Error::L7(format!("{}: {e}", path.display())))
}

/// Every runtime-valid test's four L7 cells.
fn judge(suite: &mf2_conformance::Suite, record: &Record) -> Results {
    let mut results = Results::default();
    for test in suite.tests() {
        if test.kind != TestKind::Other {
            continue;
        }
        let id = mf2_conformance::l5::id(test);
        let locale = test.locale.as_str();
        if !SETS.contains(&locale) {
            // Every cell would fail as "not on the page"; say why.
            for (_, all_column, default_column) in MODES {
                for column in [all_column, default_column] {
                    results.cells.insert(
                        (test.key.clone(), column),
                        Err(format!("no L7 set carries the locale {locale:?}")),
                    );
                }
            }
            continue;
        }
        let all = mf2_conformance::l6::check(test);
        let default = mf2_conformance::l6::check_default(test);
        for (mode, all_column, default_column) in MODES {
            let key = |column| (test.key.clone(), column);
            let cell = judge_all(&all, record, mode, locale, &id);
            results.cells.insert(key(all_column), cell);
            let (cell, degraded) = judge_default(&default, record, mode, locale, &id);
            if let Some(degraded) = degraded {
                results.degraded.insert(key(default_column), degraded);
            }
            results.cells.insert(key(default_column), cell);
        }
    }
    results
}

/// All features on: L6 must pass the test, and every engine must agree.
fn judge_all(
    l6: &Result<(), String>,
    record: &Record,
    mode: &str,
    locale: &str,
    id: &str,
) -> Result<(), String> {
    l6.clone().map_err(|e| format!("L6 fails: {e}"))?;
    if !record.on_page("all", locale, id) {
        return Err(format!("{id} is not on the {locale} page"));
    }
    record.agreed("all", mode, locale, id)
}

/// The default configuration: L6d's verdict, delivered.
fn judge_default(
    l6d: &DefaultOutcome,
    record: &Record,
    mode: &str,
    locale: &str,
    id: &str,
) -> (Result<(), String>, Option<(DegradedKind, String)>) {
    let on_page = record.on_page("default", locale, id);
    match l6d {
        DefaultOutcome::Fail(e) => (Err(format!("L6d fails: {e}")), None),
        // Refused by the build: no page in this configuration holds it.
        DefaultOutcome::Degraded(DegradedKind::BuildReject, detail) if !on_page => (
            Err(format!("degraded: build-reject: {detail}")),
            Some((DegradedKind::BuildReject, detail.clone())),
        ),
        DefaultOutcome::Degraded(DegradedKind::BuildReject, _) => (
            Err(format!(
                "{id} is on the default {locale} page, though L6d says the build refuses it"
            )),
            None,
        ),
        _ if !on_page => (
            Err(format!("{id} is not on the default {locale} page")),
            None,
        ),
        DefaultOutcome::Pass => (record.agreed("default", mode, locale, id), None),
        DefaultOutcome::Degraded(kind, detail) => {
            match record.agreed("default", mode, locale, id) {
                Ok(()) => (
                    Err(format!("degraded: {}: {detail}", kind.as_str())),
                    Some((*kind, detail.clone())),
                ),
                Err(e) => (Err(e), None),
            }
        }
    }
}
