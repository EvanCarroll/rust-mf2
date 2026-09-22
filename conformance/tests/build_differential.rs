//! `mf2-build` ≡ `mf2::compile_str` (Phase 5a, A4).
//!
//! Everything after Phase 5a formats from a catalog `mf2-build` wrote, while
//! layers L3 and L4 test catalogs `compile_str` wrote. The two paths share
//! the writer but not the road to it: `compile_str` compiles one message with
//! a one-entry manifest and the locale data that message needs;
//! `mf2-build` reads a corpus, builds a manifest from the source locale,
//! flattens fallbacks and slices the locale data over the whole set.
//!
//! So for every message of the suite, this runs both catalogs through L4's
//! runner and requires the same string, the same errors and the same parts —
//! in **both configurations**: every feature on (L4), and the default one
//! (L4d), where a gated function must instead be a **build rejection**.
//!
//! One-message corpora go through the flat JSON loader, which can carry any
//! source — including the malformed ones, which the container syntax could
//! not write.

use std::path::{Path, PathBuf};

use mf2_build::loader::json;
use mf2_build::{Build, Config, Features};
use mf2_conformance::matrix::TestKind;
use mf2_conformance::suite::SuiteTest;
use mf2_l4_runner::{Case, Config as RunConfig, Record, run};

/// The features every function needs (layer L4's registry).
fn all_features() -> Features {
    Features::parse("fn-number,fn-datetime,datetime-icu")
}

/// A directory holding one message as a one-locale corpus.
fn corpus(root: &Path, locale: &str, source: &str) -> PathBuf {
    let dir = root.join("locales");
    std::fs::create_dir_all(&dir).expect("mkdir");
    // Leave nothing from a previous message behind.
    for entry in std::fs::read_dir(&dir).expect("read_dir").flatten() {
        let _ = std::fs::remove_file(entry.path());
    }
    std::fs::write(
        dir.join(format!("{locale}.json")),
        json::write([("m", source)]),
    )
    .expect("write");
    root.to_path_buf()
}

/// Builds the one-message corpus, or gives the report's text.
fn build_one(
    root: &Path,
    out: &Path,
    test: &SuiteTest,
    features: Features,
) -> Result<(Vec<u8>, u64), String> {
    corpus(root, &test.locale, &test.src);
    let mut config = Config::default();
    config.source_locale.clone_from(&test.locale);
    // The suite names functions nothing provides on purpose, to exercise the
    // runtime's Unknown Function; that is a lint, not a catalog difference.
    // The suite exercises the *runtime's* errors on purpose: functions
    // nothing provides, `select` taken from a variable, option values that
    // are not what the option takes. mf2-two refuses all three in a real
    // corpus (plans/05 §5), but that is policy — this differential is about
    // the catalog, so the three lints are turned down here.
    for lint in [
        mf2_build::Lint::UnknownFunction,
        mf2_build::Lint::DynamicSelect,
        mf2_build::Lint::BadOptionValue,
    ] {
        config.lints.insert(lint, mf2_build::Level::Allow);
    }
    // A one-message corpus keeps its id table: nothing looks messages up by
    // name here, but stripping is L3's business, not this differential's.
    config.catalog.strip.clear();
    let outcome = Build::at(root, out)
        .config(config)
        .features(features)
        .check()
        .map_err(|e| e.to_string())?;
    if !outcome.is_clean() {
        return Err(outcome.report.to_text().trim_end().to_owned());
    }
    let catalog = outcome
        .catalog(&test.locale)
        .ok_or_else(|| format!("no catalog for {}", test.locale))?;
    Ok((catalog.bytes.clone(), outcome.manifest_hash))
}

/// What `case` formats to.
fn record(case: &Case) -> Result<Record, String> {
    run(case)
}

/// The same case, with another catalog under it.
fn like(case: &Case, catalog: Vec<u8>, manifest_hash: u64, config: RunConfig) -> Case {
    Case {
        id: case.id.clone(),
        catalog,
        manifest_hash,
        bidi: case.bidi,
        args: case.args.clone(),
        config,
    }
}

/// The repository root, as the other conformance tests find it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

#[test]
fn every_suite_message_formats_the_same_from_either_path() {
    let root = repo_root();
    let suite = mf2_conformance::load_suite(&root).expect("the suite");
    let corpus_dir = out_dir("differential-corpus");
    let out = out_dir("differential-out");

    let mut compared = 0usize;
    let mut rejected = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for test in suite.tests() {
        let built = build_one(&corpus_dir, &out, test, all_features());
        if test.kind != TestKind::Other {
            // The message is not well-formed or not valid: `compile_str`
            // refuses it, and so must the build.
            if built.is_ok() {
                problems.push(format!(
                    "{}: the build accepted a message the spec refuses",
                    test.key
                ));
            } else {
                rejected += 1;
            }
            continue;
        }
        let (theirs, _) = match mf2_conformance::l4::cases(test) {
            Ok(cases) => cases,
            Err(e) => {
                problems.push(format!("{}: compile_str: {e}", test.key));
                continue;
            }
        };
        let (bytes, hash) = match built {
            Ok(built) => built,
            Err(e) => {
                problems.push(format!("{}: mf2-build refused it: {e}", test.key));
                continue;
            }
        };
        let mine = like(&theirs, bytes, hash, RunConfig::All);
        match (record(&theirs), record(&mine)) {
            (Ok(a), Ok(b)) if a == b => compared += 1,
            (Ok(a), Ok(b)) => problems.push(format!(
                "{}: compile_str gave {:?}/{:?}, mf2-build gave {:?}/{:?}",
                test.key, a.text, a.errors, b.text, b.errors
            )),
            (a, b) => problems.push(format!("{}: run failed: {a:?} vs {b:?}", test.key)),
        }
    }

    assert!(
        problems.is_empty(),
        "{} of {} messages differ:\n{}",
        problems.len(),
        suite.tests().len(),
        problems.join("\n")
    );
    assert!(compared > 250, "only {compared} messages compared");
    assert!(rejected > 100, "only {rejected} messages rejected");
    eprintln!(
        "differential (every feature on): {compared} messages compared, \
         {rejected} refused by both, of {} in the suite",
        suite.tests().len()
    );
}

#[test]
fn a_gated_function_is_a_build_rejection_in_the_default_configuration() {
    let root = repo_root();
    let suite = mf2_conformance::load_suite(&root).expect("the suite");
    let corpus_dir = out_dir("default-corpus");
    let out = out_dir("default-out");

    let mut compared = 0usize;
    let mut rejected = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for test in suite.tests() {
        if test.kind != TestKind::Other {
            continue;
        }
        let gated = mf2_conformance::l4::GATED_FUNCTIONS
            .iter()
            .find(|(name, _)| test.src.contains(&format!(":{name}")));
        let built = build_one(&corpus_dir, &out, test, Features::default());
        match (gated, built) {
            (Some((name, feature)), Ok(_)) => problems.push(format!(
                "{}: :{name} needs `{feature}`, and the build did not say so",
                test.key
            )),
            (Some(_), Err(message)) => {
                assert!(
                    message.contains("gated-function"),
                    "{}: {message} does not name the gated function",
                    test.key
                );
                rejected += 1;
            }
            (None, Err(message)) => problems.push(format!(
                "{}: the default build refused a message that needs no feature: {message}",
                test.key
            )),
            (None, Ok((bytes, hash))) => {
                let Ok((theirs, _)) = mf2_conformance::l4::cases(test) else {
                    continue;
                };
                let theirs = like(
                    &theirs,
                    theirs.catalog.clone(),
                    theirs.manifest_hash,
                    RunConfig::Default,
                );
                let mine = like(&theirs, bytes, hash, RunConfig::Default);
                match (record(&theirs), record(&mine)) {
                    (Ok(a), Ok(b)) if a == b => compared += 1,
                    (Ok(a), Ok(b)) => problems.push(format!(
                        "{}: default: compile_str gave {:?}/{:?}, mf2-build gave {:?}/{:?}",
                        test.key, a.text, a.errors, b.text, b.errors
                    )),
                    (a, b) => problems.push(format!("{}: run failed: {a:?} vs {b:?}", test.key)),
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
    assert!(rejected > 10, "only {rejected} gated messages rejected");
    assert!(compared > 150, "only {compared} messages compared");
    eprintln!(
        "differential (default configuration): {compared} messages compared, \
         {rejected} refused for a gated function"
    );
}

/// The ledger's L4d `degraded` cells and the build agree about what the
/// default configuration cannot do (Phase 5a, A7).
///
/// `plans/01-conformance.md` §3 says a test that degrades in the default
/// configuration must name how. Two of those kinds are the build's business,
/// and this is where the two documents are held together:
///
/// * `unknown-function` — the message names a function only a feature
///   provides, so `mf2-build` must **refuse** it with `gated-function`;
/// * `neutral-numbers` — the message formats numbers without `fn-number`, so
///   `mf2 check` must **warn** `neutral-numbers` (and still build).
#[test]
fn the_ledger_and_the_build_agree_on_the_default_configuration() {
    use mf2_build::{Level, Lint};
    use mf2_conformance::ledger::{Cell, DegradedKind};
    use mf2_conformance::{Column, LEDGER_PATH, Ledger};

    let root = repo_root();
    let suite = mf2_conformance::load_suite(&root).expect("the suite");
    let text = std::fs::read_to_string(root.join(LEDGER_PATH)).expect("the ledger");
    let ledger = Ledger::parse(&text).expect("it parses");
    let corpus_dir = out_dir("ledger-corpus");
    let out = out_dir("ledger-out");

    let mut gated = 0usize;
    let mut neutral = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for entry in &ledger.entries {
        let Some(Cell::Degraded { kind, .. }) = entry.cells.get(&Column::L4d) else {
            continue;
        };
        let Some(test) = suite.tests().iter().find(|t| t.key == entry.key) else {
            continue;
        };
        match kind {
            DegradedKind::UnknownFunction => {
                gated += 1;
                match build_one(&corpus_dir, &out, test, Features::default()) {
                    Ok(_) => problems.push(format!(
                        "{}: the ledger says the default configuration cannot run \
                         this message, and the build accepted it",
                        entry.key
                    )),
                    Err(message) => {
                        if !message.contains(Lint::GatedFunction.name()) {
                            problems.push(format!(
                                "{}: refused, but not as a gated function: {message}",
                                entry.key
                            ));
                        }
                    }
                }
            }
            DegradedKind::NeutralNumbers => {
                neutral += 1;
                // The warning is on by default; the build still succeeds.
                let mut config = Config::default();
                config.source_locale.clone_from(&test.locale);
                config.catalog.strip.clear();
                for lint in [
                    Lint::UnknownFunction,
                    Lint::DynamicSelect,
                    Lint::BadOptionValue,
                ] {
                    config.lints.insert(lint, Level::Allow);
                }
                corpus(&corpus_dir, &test.locale, &test.src);
                let outcome = Build::at(&corpus_dir, &out)
                    .config(config)
                    .features(Features::default())
                    .check()
                    .expect("the corpus is readable");
                let warned = outcome
                    .report
                    .diagnostics
                    .iter()
                    .any(|d| d.lint == Some(Lint::NeutralNumbers));
                if !warned {
                    problems.push(format!(
                        "{}: the ledger says numbers degrade to neutral symbols here, \
                         and `check` did not warn:\n{}",
                        entry.key,
                        outcome.report.to_text()
                    ));
                }
            }
            // The other kinds are the runtime's, not the build's.
            DegradedKind::UnsupportedOperation | DegradedKind::BuildReject => {}
        }
    }

    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
    assert!(gated > 0, "the ledger has no L4d unknown-function cells");
    assert!(neutral > 0, "the ledger has no L4d neutral-numbers cells");
    eprintln!("ledger: {gated} gated-function and {neutral} neutral-numbers cells agreed");
}
