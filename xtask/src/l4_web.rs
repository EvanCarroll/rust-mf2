//! `cargo xtask l4-web`: conformance layer L4 in the browser for the `intl`
//! client option.
//!
//! Every L4 case `cargo xtask wasi-matches-native` runs — the suite in the all-features
//! and the default configuration, unstripped and stripped, and the
//! locale-output goldens — is compiled natively, formatted natively (the
//! Rust path) and, by `conformance/l4-web` built for
//! `wasm32-unknown-unknown` with the `intl` features, in each engine through
//! `tools/e2e/checks/l4-intl.mjs` (Chromium, Firefox, `WebKit`). Then, per
//! engine:
//!
//! * each suite test is judged against its expectations as L4 judges the
//!   native run (the stripped catalog must format as the unstripped one);
//! * each record is compared with the native one, and every difference is
//!   classified (`space`, `digits`, `symbols`, `plural`, `errors`);
//! * the differences must be exactly those the ledger records for the
//!   engine (`intl` entries of `conformance/ledger.toml`): an unrecorded
//!   difference fails, and so does a recorded one that no longer occurs.
//!
//! Outputs under `target/l4-web/`: `cases.bin`, `native.txt`,
//! `<engine>.txt` / `<engine>.json` (from the check), `report.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use mf2_catalog::{Catalog, Entry, MsgId};
use mf2_conformance::ledger::{IntlDiff, IntlKind};
use mf2_conformance::{LEDGER_PATH, Ledger, SuiteTest, TestKey, TestKind};
use mf2_l4_runner::{Case, Config, Record};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The engines, in the order `--browser all` runs them.
pub(crate) const ENGINES: [&str; 3] = ["chromium", "firefox", "webkit"];

/// What a case is.
enum Origin<'s> {
    /// A suite test, in a configuration, unstripped or stripped.
    Suite {
        test: &'s SuiteTest,
        config: Config,
        stripped: bool,
    },
    /// A locale-output golden: the Rust backends' output.
    Golden,
}

pub(crate) fn run(root: &Path, engines: &[String], build: bool) -> Result<()> {
    let suite = mf2_conformance::load_suite(root)?;
    let mut cases: Vec<Case> = Vec::new();
    let mut origins: Vec<Origin<'_>> = Vec::new();
    for test in suite.tests() {
        if test.kind != TestKind::Other {
            continue;
        }
        let (unstripped, stripped) = mf2_conformance::l4::cases(test).map_err(Error::L4)?;
        for config in [Config::All, Config::Default] {
            for (mut c, is_stripped) in [(unstripped.clone(), false), (stripped.clone(), true)] {
                c.config = config;
                c.id.push_str(if is_stripped {
                    "/stripped"
                } else {
                    "/unstripped"
                });
                if config == Config::Default {
                    c.id.push_str("/default");
                }
                cases.push(c);
                origins.push(Origin::Suite {
                    test,
                    config,
                    stripped: is_stripped,
                });
            }
        }
    }
    for family in mf2_conformance::goldens::FAMILIES {
        for g in mf2_conformance::goldens::cases(family).map_err(Error::L4)? {
            cases.push(g.case);
            origins.push(Origin::Golden);
        }
    }
    let native: Vec<Record> = cases
        .iter()
        .map(|c| mf2_l4_runner::run(c).map_err(|e| Error::L4(format!("{}: {e}", c.id))))
        .collect::<Result<_>>()?;

    let dir = root.join("target/l4-web");
    fsx::write(&dir.join("cases.bin"), &mf2_l4_runner::encode_cases(&cases))?;
    let mut native_text = String::new();
    for (c, r) in cases.iter().zip(&native) {
        let _ = writeln!(native_text, "{}\t{}", c.id, r.line());
    }
    fsx::write(&dir.join("native.txt"), native_text.as_bytes())?;

    if build {
        build_web(root, &dir)?;
        let browsers = engines.join(",");
        eprintln!(
            "l4-web: {} cases; running tools/e2e check l4-intl in {browsers}",
            cases.len()
        );
        cmd::run_inherit(
            OsStr::new("node"),
            &[
                "run.mjs",
                "l4-intl",
                "--browser",
                &browsers,
                "--json",
                "../../target/l4-web/e2e.json",
            ]
            .map(OsStr::new),
            &root.join("tools/e2e"),
        )?;
    }

    let ledger = Ledger::parse(&fsx::read_to_string(&root.join(LEDGER_PATH))?)?;
    let recorded: BTreeMap<(TestKey, &str), &IntlDiff> = ledger
        .entries
        .iter()
        .flat_map(|e| {
            e.intl.iter().flat_map(move |d| {
                d.engines
                    .iter()
                    .map(move |g| ((e.key.clone(), g.as_str()), d))
            })
        })
        .collect();

    let mut report = String::from("# L4 in the browser, the `intl` build\n\n");
    let _ = writeln!(
        report,
        "`cargo xtask l4-web`. {} cases: the suite's runtime tests in \
         both configurations, unstripped and stripped, and the locale-output goldens.\n",
        cases.len()
    );
    let mut failures = Vec::new();
    for engine in engines {
        let info = fsx::read_to_string(&dir.join(format!("{engine}.json"))).unwrap_or_default();
        let text = fsx::read_to_string(&dir.join(format!("{engine}.txt"))).map_err(|_| {
            Error::L4(format!(
                "target/l4-web/{engine}.txt missing: the check did not run"
            ))
        })?;
        let got = records(&text, &cases).map_err(|e| Error::L4(format!("{engine}: {e}")))?;
        let outcome = judge(engine, &cases, &origins, &native, &got);
        let _ = writeln!(report, "## {engine}\n\n```\n{}\n```\n", info.trim());
        let _ = writeln!(
            report,
            "L4 (all features): **{} / {}** runtime tests pass; the stripped catalogs format as the \
             unstripped ones: {}.\n",
            outcome.pass,
            outcome.pass + outcome.fail.len(),
            if outcome.stripped_differ.is_empty() {
                "all".to_owned()
            } else {
                outcome.stripped_differ.join(", ")
            }
        );
        for (id, why) in &outcome.fail {
            let _ = writeln!(report, "* fails `{id}`: {why}");
        }
        let _ = writeln!(
            report,
            "\nDifferences from the Rust path: {} suite tests ({} records), {} golden cases.\n",
            outcome.suite_diffs.len(),
            outcome.suite_records,
            outcome.golden_diffs.len()
        );
        report.push_str(
            "| test | configuration | kind | Rust path | intl |\n|---|---|---|---|---|\n",
        );
        for d in outcome.suite_diffs.values() {
            let _ = writeln!(
                report,
                "| `{}` | {} | {} | `{}` | `{}` |",
                d.id,
                d.config,
                d.kind.as_str(),
                d.native.replace('|', "\\|"),
                d.engine.replace('|', "\\|")
            );
        }
        report.push_str("\n| golden case | kind | Rust path | intl |\n|---|---|---|---|\n");
        for d in &outcome.golden_diffs {
            let _ = writeln!(
                report,
                "| `{}` | {} | `{}` | `{}` |",
                d.id,
                d.kind.as_str(),
                d.native.replace('|', "\\|"),
                d.engine.replace('|', "\\|")
            );
        }
        report.push('\n');

        // The ledger must record exactly the suite differences.
        let seen: BTreeSet<&TestKey> = outcome.suite_diffs.keys().collect();
        for (key, d) in &outcome.suite_diffs {
            match recorded.get(&(key.clone(), engine.as_str())) {
                None => failures.push(format!(
                    "{engine}: `{}` differs from the Rust path ({}: `{}` vs `{}`) and the ledger has no \
                     intl entry for it",
                    d.id,
                    d.kind.as_str(),
                    d.engine,
                    d.native
                )),
                Some(r) if r.kind != d.kind => failures.push(format!(
                    "{engine}: `{}` differs as `{}`, the ledger says `{}`",
                    d.id,
                    d.kind.as_str(),
                    r.kind.as_str()
                )),
                Some(_) => {}
            }
        }
        for ((key, g), _) in recorded.iter().filter(|((_, g), _)| *g == engine.as_str()) {
            if !seen.contains(key) {
                failures.push(format!(
                    "{g}: the ledger records an intl difference for {} #{} that no longer occurs",
                    key.file, key.hash
                ));
            }
        }
        for (id, why) in &outcome.fail {
            let recorded_fail = outcome
                .suite_diffs
                .values()
                .find(|d| &d.id == id)
                .and_then(|d| recorded.get(&(d.key.clone(), engine.as_str())))
                .is_some_and(|r| r.fails);
            if !recorded_fail {
                failures.push(format!(
                    "{engine}: `{id}` fails L4 and the ledger does not say so: {why}"
                ));
            }
        }
        for d in outcome.suite_diffs.values() {
            let fails = outcome.fail.iter().any(|(id, _)| id == &d.id);
            if let Some(r) = recorded.get(&(d.key.clone(), engine.as_str()))
                && r.fails != fails
            {
                failures.push(format!(
                    "{engine}: `{}` {} L4, the ledger says `fails = {}`",
                    d.id,
                    if fails { "fails" } else { "passes" },
                    r.fails
                ));
            }
        }
        // And exactly the goldens' differences, per group and kind.
        let mut observed: BTreeMap<(&str, IntlKind), usize> = BTreeMap::new();
        for d in &outcome.golden_diffs {
            *observed.entry((d.id.as_str(), d.kind)).or_default() += 1;
        }
        let expected: BTreeMap<(&str, IntlKind), usize> = ledger
            .intl
            .iter()
            .filter(|g| g.engines.iter().any(|e| e == engine))
            .map(|g| ((g.cases.as_str(), g.kind), g.count))
            .collect();
        for ((cases, kind), n) in &observed {
            match expected.get(&(*cases, *kind)) {
                Some(m) if m == n => {}
                Some(m) => failures.push(format!(
                    "{engine}: {n} case(s) of `{cases}` differ as `{}`, the ledger's [[intl]] says {m}",
                    kind.as_str()
                )),
                None => failures.push(format!(
                    "{engine}: {n} case(s) of `{cases}` differ from the Rust path as `{}` and the \
                     ledger has no [[intl]] entry for them",
                    kind.as_str()
                )),
            }
        }
        for (cases, kind) in expected.keys() {
            if !observed.contains_key(&(*cases, *kind)) {
                failures.push(format!(
                    "{engine}: the ledger's [[intl]] records `{}` differences in `{cases}` that no \
                     longer occur",
                    kind.as_str()
                ));
            }
        }
        for id in &outcome.stripped_differ {
            failures.push(format!(
                "{engine}: `{id}`: the stripped catalog formats differently"
            ));
        }
        eprintln!(
            "l4-web: {engine}: L4 {} / {}, {} suite tests and {} golden cases differ from the Rust path",
            outcome.pass,
            outcome.pass + outcome.fail.len(),
            outcome.suite_diffs.len(),
            outcome.golden_diffs.len()
        );
    }
    fsx::write(&dir.join("report.md"), report.as_bytes())?;
    if failures.is_empty() {
        eprintln!("l4-web: every engine difference is the ledger's (target/l4-web/report.md)");
        Ok(())
    } else {
        for f in &failures {
            eprintln!("l4-web: {f}");
        }
        Err(Error::L4(format!(
            "{} problem(s) in the browser run (target/l4-web/report.md)",
            failures.len()
        )))
    }
}

/// Builds `conformance/l4-web` for `wasm32-unknown-unknown` and binds it.
fn build_web(root: &Path, dir: &Path) -> Result<()> {
    eprintln!("l4-web: building conformance/l4-web for wasm32-unknown-unknown (the intl build)");
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            "conformance/l4-web/Cargo.toml",
            "--target-dir",
            "target/l4-web/cargo",
        ]
        .map(OsStr::new),
        root,
    )?;
    let wasm = dir.join("cargo/wasm32-unknown-unknown/release/mf2_l4_web.wasm");
    let pkg = dir.join("pkg");
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

/// An engine's records, in case order.
fn records(text: &str, cases: &[Case]) -> std::result::Result<Vec<Record>, String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() != cases.len() {
        return Err(format!("{} records for {} cases", lines.len(), cases.len()));
    }
    lines
        .iter()
        .zip(cases)
        .map(|(line, case)| {
            let (id, rest) = line.split_once('\t').ok_or("a line without an id")?;
            if id != case.id {
                return Err(format!("record `{id}` where `{}` was expected", case.id));
            }
            if let Some(e) = rest.strip_prefix("ERROR ") {
                return Err(format!("{id}: the runner failed: {e}"));
            }
            Record::from_line(rest).ok_or_else(|| format!("{id}: not a record"))
        })
        .collect()
}

/// One difference between an engine's record and the Rust path's.
struct Diff {
    id: String,
    key: TestKey,
    config: &'static str,
    kind: IntlKind,
    native: String,
    engine: String,
}

/// An engine's run, judged.
struct Outcome {
    pass: usize,
    fail: Vec<(String, String)>,
    stripped_differ: Vec<String>,
    /// Per suite test, its first difference (the all-features configuration
    /// before the default one).
    suite_diffs: BTreeMap<TestKey, Diff>,
    suite_records: usize,
    golden_diffs: Vec<Diff>,
}

fn judge(
    _engine: &str,
    cases: &[Case],
    origins: &[Origin<'_>],
    native: &[Record],
    got: &[Record],
) -> Outcome {
    let mut o = Outcome {
        pass: 0,
        fail: Vec::new(),
        stripped_differ: Vec::new(),
        suite_diffs: BTreeMap::new(),
        suite_records: 0,
        golden_diffs: Vec::new(),
    };
    for (i, origin) in origins.iter().enumerate() {
        let (case, n, g) = (&cases[i], &native[i], &got[i]);
        match origin {
            Origin::Suite {
                test,
                config,
                stripped,
            } => {
                if *stripped {
                    // The unstripped case is the one before.
                    if got.get(i.wrapping_sub(1)) != Some(g) {
                        o.stripped_differ.push(case.id.clone());
                    }
                    continue;
                }
                if *config == Config::All {
                    match mf2_conformance::l4::judge(test, g) {
                        Ok(()) => o.pass += 1,
                        Err(e) => o.fail.push((case.id.clone(), e)),
                    }
                }
                if n != g {
                    o.suite_records += 1;
                    let key = test.key.clone();
                    o.suite_diffs.entry(key.clone()).or_insert_with(|| Diff {
                        id: case.id.clone(),
                        key,
                        config: if *config == Config::All {
                            "all features"
                        } else {
                            "default"
                        },
                        kind: classify(case, n, g),
                        native: n.text.clone(),
                        engine: g.text.clone(),
                    });
                }
            }
            Origin::Golden => {
                if n != g {
                    o.golden_diffs.push(Diff {
                        id: case.id.clone(),
                        key: TestKey {
                            file: String::new(),
                            hash: String::new(),
                            nth: 0,
                        },
                        config: "all features",
                        kind: classify(case, n, g),
                        native: n.text.clone(),
                        engine: g.text.clone(),
                    });
                }
            }
        }
    }
    o
}

/// What kind of difference an engine's record has from the Rust path's.
fn classify(case: &Case, native: &Record, engine: &Record) -> IntlKind {
    if native.errors != engine.errors || native.parts_errors != engine.parts_errors {
        return IntlKind::Errors;
    }
    let spaces = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_whitespace() { ' ' } else { c })
            .collect()
    };
    if spaces(&native.text) == spaces(&engine.text) {
        // The text agrees up to space characters (U+00A0, U+202F, …): the
        // parts differ only there too, or in their kinds.
        return if native.text == engine.text {
            IntlKind::Parts
        } else {
            IntlKind::Space
        };
    }
    let digits = |s: &str| -> Vec<u32> { s.chars().filter_map(digit_value).collect() };
    if digits(&native.text) != digits(&engine.text) {
        if is_select(case) && same_skeleton(&native.text, &engine.text) {
            return IntlKind::Digits;
        }
        return if is_select(case) {
            IntlKind::Plural
        } else {
            IntlKind::Digits
        };
    }
    if is_select(case) && !same_skeleton(&native.text, &engine.text) {
        return IntlKind::Plural;
    }
    IntlKind::Symbols
}

/// The value of a decimal digit of any of the numbering systems CLDR's
/// locales default to (a run of ten from its zero).
fn digit_value(c: char) -> Option<u32> {
    const ZEROS: [u32; 12] = [
        0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xBE6, 0xE50, 0x1040, 0xFF10,
    ];
    let u = u32::from(c);
    ZEROS
        .iter()
        .find(|&&z| u >= z && u < z + 10)
        .map(|&z| u - z)
}

/// The text with every digit, space and punctuation removed: what a
/// variant's own text is made of.
fn same_skeleton(a: &str, b: &str) -> bool {
    let skel = |s: &str| -> String { s.chars().filter(|c| c.is_alphabetic()).collect() };
    skel(a) == skel(b)
}

/// Whether the case's message selects.
fn is_select(case: &Case) -> bool {
    Catalog::new(case.catalog.clone(), case.manifest_hash)
        .is_ok_and(|c| matches!(c.get(MsgId::from_raw(0)), Entry::Select(_)))
}
