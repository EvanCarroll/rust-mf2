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
/// It is built with a number formatter, so nothing here is a
/// `plain-numbers` warning until a mutation takes the formatter away.
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
            // `{#kbd}` stays: leaving it out is `dropped-markup`'s drift.
            mutate: |files, _, _| edit(files, "pl", "{#kbd}Esc{/kbd}", "{#kbd}{#b}Esc{/b}{/kbd}"),
        },
        Drift {
            lint: Lint::DateMismatch,
            what: "one language formats a variable as a date and another shows it bare",
            mutate: |files, features, _| {
                *features =
                    Features::parse("host-std-number-builtin datetime host-std-datetime-iso");
                append(files, "en", "\ndue = Due {$when :datetime}\n");
                append(files, "pl", "\ndue = Termin {$when}\n");
            },
        },
        Drift {
            lint: Lint::DroppedPlaceholder,
            what: "a translation drops a placeholder the source shows",
            mutate: |files, _, _| edit(files, "pl", "Czesc, {$name}!", "Czesc!"),
        },
        Drift {
            lint: Lint::DroppedMarkup,
            what: "a translation leaves out markup the source has",
            mutate: |files, _, _| edit(files, "pl", "{#kbd}Esc{/kbd}", "Esc"),
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
            what: "a function with no formatter on",
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
            lint: Lint::UnknownOption,
            what: "an option the function does not define",
            // `Intl`'s name, not MF2's (`dateLength`): ignored at run time.
            mutate: |files, _, _| {
                edit(
                    files,
                    "en",
                    "unit=kilometer}",
                    "unit=kilometer unitStyle=long}",
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
            lint: Lint::UnusedFeature,
            what: "`datetime` on and no message formats a date",
            mutate: |_, features, _| {
                *features =
                    Features::parse("host-std-number-builtin datetime host-std-datetime-iso");
            },
        },
        Drift {
            lint: Lint::SeveralFormatters,
            what: "two number formatters of one side",
            mutate: |_, features, _| {
                *features = Features::parse("host-std-number-plain host-std-number-builtin");
            },
        },
        Drift {
            lint: Lint::PlainNumbers,
            what: "a placeholder that can receive a number, with no number formatter",
            mutate: |files, features, _| {
                // Every number function has to go first, or that is what the
                // build would report.
                edit(
                    files,
                    "en",
                    "count =\n  .input {$n :integer}\n  .match $n\n  one {{one thing}}\n  *   {{{$n} things}}",
                    "count = {$n} things",
                );
                edit(
                    files,
                    "pl",
                    "count =\n  .input {$n :integer}\n  .match $n\n  one  {{jedna rzecz}}\n  few  {{{$n} rzeczy}}\n  many {{{$n} rzeczy}}\n  *    {{{$n} rzeczy}}",
                    "count = {$n} rzeczy",
                );
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

fn with_numbers() -> Features {
    Features::parse("host-std-number-builtin")
}

#[test]
fn the_base_corpus_is_clean() {
    let outcome = build_corpus("clean", &files(), &with_numbers(), &Config::default());
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
        let mut features = with_numbers();
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
        let mut features = with_numbers();
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
    let outcome = build_corpus("unused", &files(), &with_numbers(), &Config::default());
    let mut report = mf2_build::Report::new();
    mf2_build::check::unused_ids(
        &outcome.manifest.ids,
        "let label = tr!(\"plain\");",
        "en",
        &outcome.defined,
        Path::new("src"),
        &Config::default(),
        &mut report,
    );
    let text = report.to_text();
    assert!(text.contains("[unused-id]"), "{text}");
    assert!(text.contains("(in greeting, locale en)"), "{text}");
    assert!(
        !text.contains("(in plain,"),
        "an id a source names is used: {text}"
    );
    // Each at the file that defines it, not at the sources' directory.
    assert!(!text.contains("src:1:1"), "{text}");

    // And nothing when every id is named.
    let all = outcome.manifest.ids.join(" ");
    let mut report = mf2_build::Report::new();
    mf2_build::check::unused_ids(
        &outcome.manifest.ids,
        &all,
        "en",
        &outcome.defined,
        Path::new("src"),
        &Config::default(),
        &mut report,
    );
    assert!(report.is_empty(), "{}", report.to_text());
}

/// `dropped-markup` reads the whole message, as `dropped-placeholder` does:
/// a variant may leave the markup out while another keeps it — Polish `one`
/// says "a message" with neither the count nor its bold.
#[test]
fn markup_that_one_variant_keeps_is_not_dropped() {
    let en = "@locale en\n---\n\nnew =\n  .input {$n :integer}\n  .match $n\n  \
              one {{{#b}one{/b} new message}}\n  *   {{{#b}{$n}{/b} new messages}}\n";
    let pl_kept = "@locale pl\n---\n\nnew =\n  .input {$n :integer}\n  .match $n\n  \
                   one  {{nowa wiadomosc}}\n  few  {{{#b}{$n}{/b} nowe wiadomosci}}\n  \
                   many {{{#b}{$n}{/b} nowych wiadomosci}}\n  \
                   *    {{{#b}{$n}{/b} nowych wiadomosci}}\n";
    let files = |pl: &str| {
        vec![
            ("en".to_owned(), en.to_owned()),
            ("pl".to_owned(), pl.to_owned()),
        ]
    };
    let outcome = build_corpus(
        "markup-kept",
        &files(pl_kept),
        &with_numbers(),
        &Config::default(),
    );
    assert!(
        outcome.report.is_empty(),
        "a variant may leave markup out:\n{}",
        outcome.report.to_text()
    );

    // Left out of every variant, it is reported, by name.
    let pl_dropped = pl_kept.replace("{#b}", "").replace("{/b}", "");
    let outcome = build_corpus(
        "markup-dropped",
        &files(&pl_dropped),
        &with_numbers(),
        &Config::default(),
    );
    let text = outcome.report.to_text();
    assert!(text.contains("{#b}"), "{text}");
    assert!(text.contains("[dropped-markup]"), "{text}");
    assert_eq!(outcome.report.errors(), 1, "{text}");
}

/// The UX review's case (Phase 10 E2): French had one of four messages, and
/// one of the three it lacked was a language's own name, marked
/// `@do-not-translate`. It needs no translation, so French is missing two
/// of three — not three of four — and a copy of it is not a translation.
#[test]
fn do_not_translate_messages_are_neither_missing_nor_covered() {
    let en = "@locale en\n---\n\ngreeting = Hello\nfarewell = Goodbye\napply = Apply\n\n\
              @do-not-translate\nlanguage-fr = Français\n";
    let files = |fr: &str| {
        vec![
            ("en".to_owned(), en.to_owned()),
            ("fr".to_owned(), fr.to_owned()),
        ]
    };
    let config = Config::default();
    let outcome = build_corpus(
        "dnt-missing",
        &files("@locale fr\n---\n\ngreeting = Bonjour\n"),
        &with_numbers(),
        &config,
    );
    let text = outcome.report.to_text();
    assert!(
        text.contains("2 of 3 messages are missing here and fall back to en: apply, farewell"),
        "{text}"
    );
    let fr = &outcome.coverage[1];
    assert_eq!(
        (fr.tag.as_str(), fr.translatable, fr.translated()),
        ("fr", 3, 1)
    );
    assert_eq!(outcome.coverage[0].missing, Vec::<String>::new());

    // A copy of the language's name is not a translation: still one of
    // three.
    let outcome = build_corpus(
        "dnt-copied",
        &files("@locale fr\n---\n\ngreeting = Bonjour\nlanguage-fr = Français\n"),
        &with_numbers(),
        &config,
    );
    assert!(
        outcome
            .report
            .to_text()
            .contains("2 of 3 messages are missing here"),
        "{}",
        outcome.report.to_text()
    );
    assert_eq!(outcome.coverage[1].translated(), 1);

    // `@do-not-translate` on a section covers its entries, as XLIFF's
    // `translate="no"` on a group does.
    let en = "@locale en\n---\n\ngreeting = Hello\n\n@do-not-translate\n[language]\n\
              en = English\nfr = Français\n";
    let outcome = build_corpus(
        "dnt-section",
        &[
            ("en".to_owned(), en.to_owned()),
            (
                "fr".to_owned(),
                "@locale fr\n---\n\ngreeting = Bonjour\n".to_owned(),
            ),
        ],
        // Plain text only: with a number formatter on, that is
        // `unused-feature`.
        &Features::default(),
        &config,
    );
    assert!(outcome.report.is_empty(), "{}", outcome.report.to_text());
    assert_eq!(outcome.coverage[1].translatable, 1);
}

/// The features `unused-feature` reported, in the order it reported them.
fn unused(files: &[(String, String)], features: &str) -> Vec<String> {
    let name = format!("unused-feature-{}", features.replace(' ', "-"));
    let outcome = build_corpus(&name, files, &Features::parse(features), &Config::default());
    outcome
        .report
        .diagnostics
        .iter()
        .filter(|d| d.lint == Some(Lint::UnusedFeature))
        .map(|d| d.message.clone())
        .collect()
}

/// A corpus of one locale, `en`, with `body` for its messages.
fn only_en(body: &str) -> Vec<(String, String)> {
    vec![("en".to_owned(), format!("@locale en\n---\n\n{body}"))]
}

#[test]
fn unused_feature_names_each_family_once() {
    // Plain text: no placeholder, no function, no selection.
    let text = only_en("a = Save\nb = Cancel\n");
    let said = unused(
        &text,
        "host-web-number-intl host-std-number-builtin datetime host-std-datetime-iso",
    );
    assert_eq!(said.len(), 2, "{said:#?}");
    assert!(
        said[0]
            .contains("`host-web-number-intl` and `host-std-number-builtin` are on for this build"),
        "{}",
        said[0]
    );
    assert!(
        said[1].contains("`host-std-datetime-iso` is on for this build"),
        "{}",
        said[1]
    );
    assert!(
        said[1].contains("a date handed to a plain placeholder is an error"),
        "{}",
        said[1]
    );
    // The base corpus formats numbers and no dates; across two locales the
    // date family is still reported once.
    let said = unused(&files(), "native native-number-builtin native-datetime-iso");
    assert_eq!(said.len(), 1, "{said:#?}");
    assert!(
        said[0].contains("`native-datetime-iso` is on for this build"),
        "{}",
        said[0]
    );
    // `datetime` alone is named as itself, and so is `number`.
    let said = unused(&text, "datetime");
    assert_eq!(said.len(), 1, "{said:#?}");
    assert!(
        said[0].contains("`datetime` is on for this build"),
        "{}",
        said[0]
    );
    let said = unused(&text, "number");
    assert_eq!(said.len(), 1, "{said:#?}");
    assert!(
        said[0].contains("`number` is on for this build"),
        "{}",
        said[0]
    );
}

#[test]
fn unused_feature_names_a_family_without_its_framework() {
    // The base corpus formats no date, so the formatter is unused too; the
    // framework's line is the second.
    let said = unused(
        &files(),
        "host-std host-std-number-builtin axum-datetime-iso",
    );
    assert_eq!(said.len(), 2, "{said:#?}");
    assert!(
        said[1].contains(
            "`axum-datetime-iso` is on for this build and its framework is not: it is the \
             date formatter of a framework this crate does not use"
        ),
        "{}",
        said[1]
    );
    // With its framework on, only the unused formatter is said.
    let said = unused(&files(), "axum axum-number-builtin axum-datetime-iso");
    assert_eq!(said.len(), 1, "{said:#?}");
    // A number formatter's family without its framework says the same of
    // itself; the corpus formats numbers, so it is the only line.
    let said = unused(&files(), "host-std native-number-builtin");
    assert_eq!(said.len(), 1, "{said:#?}");
    assert!(
        said[0].contains(
            "`native-number-builtin` is on for this build and its framework is not: it is \
             the number formatter of a framework this crate does not use"
        ),
        "{}",
        said[0]
    );
    // A host family needs no framework.
    let dates = only_en("when = {$at :date}\n");
    assert!(unused(&dates, "host-std host-std-datetime-iso").is_empty());
}

/// The messages `lint` gave for `body` under `features`.
fn said(lint: Lint, name: &str, body: &str, features: &str) -> Vec<String> {
    let outcome = build_corpus(
        name,
        &only_en(body),
        &Features::parse(features),
        &Config::default(),
    );
    outcome
        .report
        .diagnostics
        .iter()
        .filter(|d| d.lint == Some(lint))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn a_date_function_with_no_formatter_names_the_features_to_write() {
    let body = "when = {$at :date}\n";
    // `datetime` alone is no formatter (`plan/08` §3.3).
    let alone = said(
        Lint::GatedFunction,
        "gated-datetime-alone",
        body,
        "datetime",
    );
    assert_eq!(alone.len(), 1, "{alone:#?}");
    assert!(alone[0].contains("`native-datetime-icu`"), "{}", alone[0]);
    // A server-rendered build that names only the browser's formatter.
    let ssr = said(
        Lint::GatedFunction,
        "gated-ssr-client-only",
        body,
        "leptos ssr leptos-client-datetime-intl",
    );
    assert_eq!(ssr.len(), 1, "{ssr:#?}");
    assert!(
        ssr[0].contains("Write `leptos-server-datetime-icu`"),
        "{}",
        ssr[0]
    );
    assert!(ssr[0].contains("+298 KB"), "{}", ssr[0]);
    // With the server's formatter too, it builds.
    assert!(
        said(
            Lint::GatedFunction,
            "gated-ssr-both",
            body,
            "leptos ssr leptos-client-datetime-intl leptos-server-datetime-icu",
        )
        .is_empty()
    );
    // A command-line tool and an Axum server get their own family's line.
    let cli = said(Lint::GatedFunction, "gated-native", body, "native");
    assert!(cli[0].contains("Write `native-datetime-icu`"), "{}", cli[0]);
    let axum = said(Lint::GatedFunction, "gated-axum", body, "axum");
    assert!(axum[0].contains("Write `axum-datetime-icu`"), "{}", axum[0]);
}

#[test]
fn several_formatters_names_the_one_that_formats() {
    let body = "when = {$at :date}\n";
    let both = said(
        Lint::SeveralFormatters,
        "several-native",
        body,
        "native native-datetime-iso native-datetime-icu",
    );
    assert_eq!(both.len(), 1, "{both:#?}");
    assert!(both[0].contains("for native code"), "{}", both[0]);
    assert!(both[0].contains("that is `icu` (ICU4X)"), "{}", both[0]);
    let browser = said(
        Lint::SeveralFormatters,
        "several-browser",
        body,
        "leptos csr leptos-client-datetime-iso leptos-client-datetime-intl",
    );
    assert!(browser[0].contains("for the browser"), "{}", browser[0]);
    assert!(browser[0].contains("that is `intl`"), "{}", browser[0]);
    // One formatter on each side is not several.
    assert!(
        said(
            Lint::SeveralFormatters,
            "several-split",
            body,
            "leptos ssr leptos-client-datetime-intl leptos-server-datetime-icu",
        )
        .is_empty()
    );
    // Numbers, the same: the strongest of a side's formats.
    let count = "n = {$n :integer}\n";
    let numbers = said(
        Lint::SeveralFormatters,
        "several-numbers",
        count,
        "leptos csr leptos-client-number-intl leptos-client-number-builtin",
    );
    assert_eq!(numbers.len(), 1, "{numbers:#?}");
    assert!(
        numbers[0].contains(
            "`leptos-client-number-builtin` and `leptos-client-number-intl` are on for this \
             build, 2 number formatters for the browser"
        ),
        "{}",
        numbers[0]
    );
    assert!(
        numbers[0].contains("that is `builtin` (mf2's own code over the catalog's CLDR data)"),
        "{}",
        numbers[0]
    );
    assert!(
        said(
            Lint::SeveralFormatters,
            "several-numbers-split",
            count,
            "leptos ssr leptos-client-number-intl leptos-server-number-builtin",
        )
        .is_empty()
    );
}

#[test]
fn a_number_function_with_no_formatter_names_the_features_to_write() {
    let body = "n = {$n :integer}\n";
    // No number formatter at all.
    let none = said(Lint::GatedFunction, "gated-number-none", body, "native");
    assert_eq!(none.len(), 1, "{none:#?}");
    assert!(
        none[0].contains(":integer formats a number, and this build has no number formatter"),
        "{}",
        none[0]
    );
    assert!(
        none[0].contains("Write `native-number-builtin`"),
        "{}",
        none[0]
    );
    // `plain` formats it, and no `:currency`.
    assert!(
        said(
            Lint::GatedFunction,
            "gated-number-plain",
            body,
            "native native-number-plain",
        )
        .is_empty()
    );
    let money = said(
        Lint::GatedFunction,
        "gated-currency-plain",
        "price = {$n :currency currency=EUR}\n",
        "native native-number-plain",
    );
    assert_eq!(money.len(), 1, "{money:#?}");
    assert!(
        money[0].contains("(`native-number-plain` is plain digits)"),
        "{}",
        money[0]
    );
    assert!(
        money[0].contains("Write `native-number-builtin`"),
        "{}",
        money[0]
    );
}

#[test]
fn a_bare_placeholder_with_no_number_formatter_is_plain_numbers() {
    let body = "hello = Hello, {$name}!\n";
    let none = said(Lint::PlainNumbers, "plain-numbers-none", body, "native");
    assert_eq!(none.len(), 1, "{none:#?}");
    assert!(
        none[0].contains("this build has no number formatter for native code"),
        "{}",
        none[0]
    );
    assert!(
        none[0].contains("Write `native-number-builtin` on the `mf2` dependency"),
        "{}",
        none[0]
    );
    // A side that chose `plain` chose plain digits: nothing to say.
    for features in ["native native-number-plain", "native native-number-builtin"] {
        assert!(
            said(Lint::PlainNumbers, "plain-numbers-chosen", body, features).is_empty(),
            "{features}"
        );
    }
    // A corpus with no bare placeholder has nothing that could print one.
    assert!(
        said(
            Lint::PlainNumbers,
            "plain-numbers-text",
            "save = Save\n",
            "native"
        )
        .is_empty()
    );
}

#[test]
fn unused_feature_is_silent_where_the_feature_is_used_or_off() {
    let text = only_en("a = Save\n");
    assert!(unused(&text, "").is_empty());
    // Any of the three date functions uses the family.
    for function in ["datetime", "date", "time"] {
        let dates = only_en(&format!("when = {{$at :{function}}}\n"));
        let said = unused(&dates, "datetime host-web-datetime-intl");
        assert!(said.is_empty(), ":{function}: {said:#?}");
    }
    // A plain placeholder can receive a number.
    let plain = only_en("hello = Hello, {$name}!\n");
    assert!(unused(&plain, "host-std-number-builtin").is_empty());
    // So does a numeric function, and a plural selection with no placeholder.
    for function in ["{$n :integer}", "{$n :percent}", "{$n :unit unit=meter}"] {
        let numeric = only_en(&format!("n = {function}\n"));
        let said = unused(&numeric, "host-web-number-intl host-std-number-builtin");
        assert!(said.is_empty(), "{function}: {said:#?}");
    }
    let select =
        only_en("count =\n  .input {$n :number}\n  .match $n\n  one {{one}}\n  * {{many}}\n");
    assert!(unused(&select, "host-std-number-builtin").is_empty());
    // `allow` silences it.
    let mut config = Config::default();
    config.lints.insert(Lint::UnusedFeature, Level::Allow);
    let outcome = build_corpus(
        "unused-feature-allow",
        &text,
        &Features::parse("host-std-number-builtin datetime"),
        &config,
    );
    assert!(
        outcome
            .report
            .diagnostics
            .iter()
            .all(|d| d.lint != Some(Lint::UnusedFeature))
    );
}

/// `date-mismatch` (`plan/08` §4.3): a variable one language formats with a
/// date function and another shows bare refuses the build, whichever of the
/// two is the source, and through a declaration too; a variable both format
/// as a date, or both show bare, is no mismatch.
#[test]
fn date_mismatch_compares_each_translation_with_the_source() {
    let corpus = |en: &str, pl: &str| -> Vec<(String, String)> {
        vec![
            ("en".to_owned(), format!("@locale en\n---\n\n{en}\n")),
            ("pl".to_owned(), format!("@locale pl\n---\n\n{pl}\n")),
        ]
    };
    let features = Features::parse("host-std-number-builtin datetime host-std-datetime-iso");
    let mismatches = |name: &str, en: &str, pl: &str| -> usize {
        let outcome = build_corpus(name, &corpus(en, pl), &features, &Config::default());
        outcome
            .report
            .diagnostics
            .iter()
            .filter(|d| d.lint == Some(Lint::DateMismatch) && d.level == Level::Error)
            .count()
    };
    // The source formats it as a date, the translation shows it bare.
    assert_eq!(
        mismatches(
            "date-mismatch-source",
            "due = Due {$when :datetime}",
            "due = Termin {$when}"
        ),
        1
    );
    // The other way round.
    assert_eq!(
        mismatches(
            "date-mismatch-translation",
            "due = Due {$when}",
            "due = Termin {$when :date}"
        ),
        1
    );
    // Through a declaration: `.input {$when :time}` formats it before the
    // pattern, so its bare placeholder is a date.
    assert_eq!(
        mismatches(
            "date-mismatch-declared",
            "due = .input {$when :time} {{Due {$when}}}",
            "due = Termin {$when}"
        ),
        1
    );
    // Both as dates, through different functions and a `.local`: none.
    assert_eq!(
        mismatches(
            "date-mismatch-none",
            "due = Due {$when :datetime}",
            "due = .local $d = {$when :date} {{Termin {$d}}}"
        ),
        0
    );
    // Both bare: none (a date there is a Bad Operand in both, alike).
    assert_eq!(
        mismatches(
            "date-mismatch-bare",
            "due = Due {$when}",
            "due = Termin {$when}"
        ),
        0
    );
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
        .features(with_numbers())
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
