//! The seeded-drift corpus (Phase 5a, A7): one mutation per lint class.
//!
//! Every lint has to do two things — fire on the drift that is its own, and
//! stay quiet on a clean corpus. A table of mutations proves both at once:
//! each is applied to the same clean base, and the run must report that lint
//! and no other error.
//!
//! The base is small and hand-written so that the table reads as the lints'
//! documentation; `the_reference_workload_is_clean` then says the same thing
//! about the 6,400 real messages.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{out_dir, workload};
use mf2_build::{Build, Config, Features, Level, Lint};

/// The clean corpus every mutation starts from.
///
/// It is built with `fn-number`, so nothing here is a `neutral-numbers`
/// warning until a mutation turns that feature off.
fn base() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "en",
            "\
@locale en
---

plain = Save
greeting = Hello, {$name}!

@param $n - How many.
count =
  .input {$n :integer}
  .match $n
  one {{one thing}}
  *   {{{$n} things}}

markup = Press {#kbd}Esc{/kbd} to close
priced = It costs {$amount :currency currency=EUR}
measured = {$distance :unit unit=kilometer}

@do-not-translate
brand = Example
",
        ),
        (
            "pl",
            "\
@locale pl
---

plain = Zapisz
greeting = Czesc, {$name}!

count =
  .input {$n :integer}
  .match $n
  one  {{jedna rzecz}}
  few  {{{$n} rzeczy}}
  many {{{$n} rzeczy}}
  *    {{{$n} rzeczy}}

markup = Nacisnij {#kbd}Esc{/kbd}, aby zamknac
priced = Kosztuje {$amount :currency currency=EUR}
measured = {$distance :unit unit=kilometer}
brand = Example
",
        ),
    ]
}

/// One seeded drift: what it is, and what it does to the corpus.
struct Drift {
    lint: Lint,
    what: &'static str,
    /// Edits `(locale, text)` in place; may also change the features.
    mutate: fn(&mut Vec<(String, String)>, &mut Features, &mut Config),
}

/// Replaces the first occurrence of `from` in `locale`'s text.
fn edit(files: &mut [(String, String)], locale: &str, from: &str, to: &str) {
    let file = files
        .iter_mut()
        .find(|(tag, _)| tag == locale)
        .unwrap_or_else(|| panic!("no locale {locale}"));
    assert!(file.1.contains(from), "{locale} has no {from:?}");
    file.1 = file.1.replacen(from, to, 1);
}

/// Appends to `locale`'s text.
fn append(files: &mut [(String, String)], locale: &str, text: &str) {
    let file = files
        .iter_mut()
        .find(|(tag, _)| tag == locale)
        .unwrap_or_else(|| panic!("no locale {locale}"));
    file.1.push_str(text);
}

fn drifts() -> Vec<Drift> {
    vec![
        Drift {
            lint: Lint::ExtraId,
            what: "a translation invents an id",
            mutate: |files, _, _| append(files, "pl", "\nmystery = Co?\n"),
        },
        Drift {
            lint: Lint::UndeclaredVariable,
            what: "a translation uses a variable the source does not declare",
            mutate: |files, _, _| edit(files, "pl", "Czesc, {$name}!", "Czesc, {$other}!"),
        },
        Drift {
            lint: Lint::UndeclaredMarkup,
            what: "a translation uses markup the source does not",
            mutate: |files, _, _| edit(files, "pl", "{#kbd}Esc{/kbd}", "{#b}Esc{/b}"),
        },
        Drift {
            lint: Lint::DroppedPlaceholder,
            what: "a translation drops a placeholder the source shows",
            mutate: |files, _, _| edit(files, "pl", "Czesc, {$name}!", "Czesc!"),
        },
        Drift {
            lint: Lint::DoNotTranslate,
            what: "a message marked @do-not-translate differs",
            mutate: |files, _, _| edit(files, "pl", "brand = Example", "brand = Przyklad"),
        },
        Drift {
            lint: Lint::DuplicateId,
            what: "one locale defines an id twice",
            mutate: |files, _, _| append(files, "en", "\nplain = Store\n"),
        },
        Drift {
            lint: Lint::LocaleMismatch,
            what: "a file declares another locale than its directory",
            mutate: |files, _, _| edit(files, "pl", "@locale pl", "@locale de"),
        },
        Drift {
            lint: Lint::MissingTranslation,
            what: "a translation lacks a message",
            mutate: |files, _, _| edit(files, "pl", "plain = Zapisz\n", ""),
        },
        Drift {
            lint: Lint::MissingPluralCategory,
            what: "a plural match does not name every category the locale has",
            mutate: |files, _, _| edit(files, "pl", "  few  {{{$n} rzeczy}}\n", ""),
        },
        Drift {
            lint: Lint::UnpairedMarkup,
            what: "markup is opened and never closed",
            mutate: |files, _, _| edit(files, "en", "{#kbd}Esc{/kbd}", "{#kbd}Esc"),
        },
        Drift {
            lint: Lint::GatedFunction,
            what: "a function whose feature is off",
            mutate: |_, features, _| *features = Features::default(),
        },
        Drift {
            lint: Lint::UnknownFunction,
            what: "a function nothing provides",
            mutate: |files, _, _| edit(files, "en", "plain = Save", "plain = {:app:emoji}"),
        },
        Drift {
            lint: Lint::BadOptionValue,
            what: "an option value the option cannot take",
            mutate: |files, _, _| {
                edit(
                    files,
                    "en",
                    "currency=EUR}",
                    "currency=EUR minimumFractionDigits=lots}",
                );
            },
        },
        Drift {
            lint: Lint::DynamicSelect,
            what: "`select` taken from a variable",
            mutate: |files, _, _| edit(files, "en", "{$n :integer}", "{$n :integer select=$how}"),
        },
        Drift {
            lint: Lint::DynamicCurrency,
            what: "`:currency` whose currency comes from a variable",
            mutate: |files, _, _| edit(files, "en", "currency=EUR", "currency=$code"),
        },
        Drift {
            lint: Lint::DynamicUnit,
            what: "`:unit` whose unit comes from a variable",
            mutate: |files, _, _| edit(files, "en", "unit=kilometer", "unit=$how"),
        },
        Drift {
            lint: Lint::NeutralNumbers,
            what: "numbers formatted with `fn-number` off",
            mutate: |files, features, _| {
                // The gated functions have to go first, or that is what the
                // build would report.
                edit(
                    files,
                    "en",
                    "priced = It costs {$amount :currency currency=EUR}",
                    "priced = It costs {$amount}",
                );
                edit(
                    files,
                    "en",
                    "measured = {$distance :unit unit=kilometer}",
                    "measured = {$distance}",
                );
                edit(
                    files,
                    "pl",
                    "priced = Kosztuje {$amount :currency currency=EUR}",
                    "priced = Kosztuje {$amount}",
                );
                edit(
                    files,
                    "pl",
                    "measured = {$distance :unit unit=kilometer}",
                    "measured = {$distance}",
                );
                *features = Features::default();
            },
        },
        Drift {
            lint: Lint::NonNfcSource,
            what: "source text that is not in NFC",
            // "e" + U+0301 COMBINING ACUTE ACCENT, which NFC composes to "é".
            mutate: |files, _, _| edit(files, "en", "plain = Save", "plain = Sa\u{301}ve"),
        },
        Drift {
            lint: Lint::SuspiciousBidi,
            what: "an isolate that is opened and never closed",
            mutate: |files, _, _| edit(files, "en", "plain = Save", "plain = \u{2066}Save"),
        },
        Drift {
            lint: Lint::NonstandardName,
            what: "a variable name that mixes scripts",
            // The second letter is U+0430 CYRILLIC SMALL LETTER A: valid MF2,
            // and indistinguishable from `$name` on the page.
            mutate: |files, _, _| {
                edit(files, "en", "{$name}", "{$n\u{430}me}");
                edit(files, "pl", "{$name}", "{$n\u{430}me}");
            },
        },
        Drift {
            lint: Lint::UnusedId,
            what: "an id no source file names",
            // The scan is the command line's; the lint is checked directly
            // below, with sources that name only one of the ids.
            mutate: |_, _, _| {},
        },
    ]
}

/// Writes a corpus and builds it.
fn build_corpus(
    name: &str,
    files: &[(String, String)],
    features: &Features,
    config: &Config,
) -> mf2_build::Outcome {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("drift")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    for (tag, text) in files {
        let path = root.join("locales").join(tag).join("main.mf2");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
    }
    Build::at(&root, out_dir(&format!("drift-{name}")))
        .config(config.clone())
        .features(features.clone())
        .check()
        .expect("the corpus is readable")
}

fn files() -> Vec<(String, String)> {
    base()
        .into_iter()
        .map(|(tag, text)| (tag.to_owned(), text.to_owned()))
        .collect()
}

fn with_fn_number() -> Features {
    Features::parse("fn-number")
}

#[test]
fn the_base_corpus_is_clean() {
    let outcome = build_corpus("clean", &files(), &with_fn_number(), &Config::default());
    assert!(
        outcome.report.is_empty(),
        "the base corpus is not clean:\n{}",
        outcome.report.to_text()
    );
}

#[test]
fn every_lint_fires_on_its_own_drift_and_nothing_else_does() {
    for drift in drifts() {
        if drift.lint == Lint::UnusedId {
            continue; // checked below: it needs the application's sources.
        }
        let mut files = files();
        let mut features = with_fn_number();
        let mut config = Config::default();
        (drift.mutate)(&mut files, &mut features, &mut config);
        let name = drift.lint.name();
        let outcome = build_corpus(name, &files, &features, &config);
        let fired: BTreeSet<Lint> = outcome
            .report
            .diagnostics
            .iter()
            .filter_map(|d| d.lint)
            .collect();
        assert!(
            fired.contains(&drift.lint),
            "{name} ({}) did not fire; the run said:\n{}",
            drift.what,
            outcome.report.to_text()
        );
        // Nothing else may be an *error*: a drift seeded for one lint must
        // not be reported as another.
        let other_errors: Vec<&mf2_build::Diagnostic> = outcome
            .report
            .diagnostics
            .iter()
            .filter(|d| d.level == Level::Error && d.lint != Some(drift.lint))
            .collect();
        assert!(
            other_errors.is_empty(),
            "{name} also reported {other_errors:#?}"
        );
    }
}

#[test]
fn a_lint_set_to_allow_says_nothing() {
    for drift in drifts() {
        if drift.lint == Lint::UnusedId || drift.lint.floor() == Level::Error {
            continue;
        }
        let mut files = files();
        let mut features = with_fn_number();
        let mut config = Config::default();
        (drift.mutate)(&mut files, &mut features, &mut config);
        config.lints.insert(drift.lint, Level::Allow);
        let outcome = build_corpus(
            &format!("{}-allowed", drift.lint.name()),
            &files,
            &features,
            &config,
        );
        assert!(
            !outcome
                .report
                .diagnostics
                .iter()
                .any(|d| d.lint == Some(drift.lint)),
            "{} still fired when it was allowed",
            drift.lint
        );
    }
}

#[test]
fn unused_id_fires_on_the_ids_no_source_names() {
    let outcome = build_corpus("unused", &files(), &with_fn_number(), &Config::default());
    let mut report = mf2_build::Report::new();
    mf2_build::check::unused_ids(
        &outcome.manifest.ids,
        "let label = tr!(\"plain\");",
        "en",
        Path::new("src"),
        &Config::default(),
        &mut report,
    );
    let text = report.to_text();
    assert!(text.contains("[unused-id]"), "{text}");
    assert!(text.contains("greeting"), "{text}");
    assert!(
        !text.contains(" plain,"),
        "an id a source names is used: {text}"
    );

    // And nothing when every id is named.
    let all = outcome.manifest.ids.join(" ");
    let mut report = mf2_build::Report::new();
    mf2_build::check::unused_ids(
        &outcome.manifest.ids,
        &all,
        "en",
        Path::new("src"),
        &Config::default(),
        &mut report,
    );
    assert!(report.is_empty(), "{}", report.to_text());
}

#[test]
fn every_lint_has_a_seeded_drift() {
    let seeded: BTreeSet<Lint> = drifts().into_iter().map(|d| d.lint).collect();
    let missing: Vec<&Lint> = Lint::ALL.iter().filter(|l| !seeded.contains(l)).collect();
    assert!(missing.is_empty(), "no drift seeded for {missing:?}");
}

#[test]
fn the_reference_workload_is_clean() {
    let root = workload();
    let outcome = Build::at(&root, out_dir("drift-workload"))
        .features(with_fn_number())
        .check()
        .expect("the workload is readable");
    assert!(
        outcome.report.is_clean(),
        "the reference workload has errors:\n{}",
        outcome.report.to_text()
    );
    // The only warnings are the ones the pseudo-locales earn: they carry the
    // source's plural variants, which Arabic's six categories do not match.
    let lints: BTreeSet<Option<Lint>> = outcome.report.diagnostics.iter().map(|d| d.lint).collect();
    assert_eq!(
        lints,
        [Some(Lint::MissingPluralCategory)].into_iter().collect(),
        "{}",
        outcome.report.to_text()
    );
}

/// The lints' names, for a reader of the report.
#[test]
fn the_drift_table_documents_what_each_lint_is_for() {
    for drift in drifts() {
        assert!(!drift.what.is_empty(), "{} has no description", drift.lint);
    }
}
