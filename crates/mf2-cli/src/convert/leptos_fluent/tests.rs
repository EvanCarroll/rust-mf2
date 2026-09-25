//! The reference application, converted (`plans/16-phase-8-work-order.md`
//! A4): `fluent-view` — the reference workload's call sites in
//! `leptos-fluent`'s idiom — rewritten by §6.2's rules must be, byte for
//! byte, `fluent-converted`: `tr-view` with each documented difference
//! written as its own row. And the negative control: a rule switched off
//! leaves its sites reported, and the comparison fails.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use workload_gen::sites::{Mode, Shape, SitePlan};
use workload_gen::template::Template;
use workload_gen::{Format, Knobs, Workload};

use super::rewrite::{Options, Rule};
use super::{Change, messages, rewrite_all};
use crate::convert::fluent;
use crate::convert::report::{Code, Report};

fn template(name: &str) -> Template {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/workload-gen/templates")
        .join(name);
    Template::load(&dir).expect("the template")
}

/// The generated workload: `path` → text.
fn generated() -> BTreeMap<String, String> {
    let files = workload_gen::generate_as(
        &Knobs::default(),
        &[Format::Ftl],
        &[template("fluent-view"), template("fluent-converted")],
    )
    .expect("the workload");
    files
        .iter()
        .map(|(path, bytes)| {
            (
                path.to_owned(),
                String::from_utf8(bytes.to_vec()).expect("utf-8"),
            )
        })
        .collect()
}

/// §6.1 on the workload's `.ftl` files, in memory.
fn converted(files: &BTreeMap<String, String>) -> Vec<(PathBuf, String)> {
    let mut by_locale: BTreeMap<&str, Vec<fluent::File>> = BTreeMap::new();
    for (path, text) in files {
        let Some(rest) = path.strip_prefix("ftl/") else {
            continue;
        };
        let (tag, name) = rest.split_once('/').expect("ftl/<tag>/<ns>.ftl");
        by_locale.entry(tag).or_default().push(fluent::File {
            path: PathBuf::from(path),
            name: name.trim_end_matches(".ftl").to_owned(),
            text: text.clone(),
        });
    }
    let mut report = Report::default();
    let mut outputs = Vec::new();
    for (tag, files) in &by_locale {
        for out in fluent::convert_locale(tag, files, fluent::Options::default(), &mut report) {
            outputs.push((
                Path::new("locales")
                    .join(tag)
                    .join(format!("{}.mf2", out.name)),
                out.text,
            ));
        }
    }
    assert_eq!(report.errors(), 0, "{}", report.to_text());
    outputs
}

/// The application's Rust files, rewritten with `options`.
fn rewritten(
    files: &BTreeMap<String, String>,
    options: &Options,
) -> (BTreeMap<String, String>, Report) {
    let outputs = converted(files);
    let messages = messages(&outputs, Path::new("locales"), "en");
    let sources: Vec<(PathBuf, String)> = files
        .iter()
        .filter(|(path, _)| {
            path.starts_with("app-fluent-view/src/")
                && Path::new(path).extension().is_some_and(|e| e == "rs")
        })
        .map(|(path, text)| (PathBuf::from(path), text.clone()))
        .collect();
    let mut report = Report::default();
    let changes: Vec<Change> = rewrite_all(&sources, &messages, options, &mut report);
    let mut out: BTreeMap<String, String> = sources
        .into_iter()
        .map(|(path, text)| (path.display().to_string(), text))
        .collect();
    for change in changes {
        out.insert(change.path.display().to_string(), change.after);
    }
    (out, report)
}

/// The files with call sites that differ from `fluent-converted`'s.
fn differing(
    files: &BTreeMap<String, String>,
    rewritten: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut compared = 0;
    let mut differ = Vec::new();
    for (path, text) in rewritten {
        let rel = path.trim_start_matches("app-fluent-view/");
        if !(rel.starts_with("src/components/") || rel == "src/tables.rs") {
            continue;
        }
        let expected = &files[&format!("app-fluent-converted/{rel}")];
        compared += 1;
        if text != expected {
            differ.push(rel.to_owned());
        }
    }
    assert!(compared > 60, "{compared} files compared");
    differ
}

fn options(without: Option<Rule>) -> Options {
    Options {
        i18n_crate: Some("workload_i18n".to_owned()),
        without,
    }
}

#[test]
fn the_reference_application_converts_to_fluent_converted() {
    let files = generated();
    let (rewritten, report) = rewritten(&files, &options(None));
    assert_eq!(differing(&files, &rewritten), Vec::<String>::new());

    // What is left by hand is the initializer and the `use` beside it —
    // nothing at a call site.
    let codes: Vec<(Code, String)> = report
        .findings
        .iter()
        .map(|f| (f.code, f.file.display().to_string()))
        .collect();
    let support = "app-fluent-view/src/support.rs".to_owned();
    assert_eq!(
        codes,
        vec![
            (Code::LfImport, support.clone()),
            (Code::LfInitializer, support)
        ],
        "{}",
        report.to_text()
    );

    // The documented differences, counted from the site plan and found in
    // the rewritten text.
    let wl = Workload::generate(&Knobs::default()).expect("the workload");
    let plan = SitePlan::generate(&wl).expect("the plan");
    let count =
        |f: &dyn Fn(Shape, Mode) -> bool| plan.sites.iter().filter(|s| f(s.shape, s.mode)).count();
    let signal = count(&|_, m| matches!(m, Mode::Get | Mode::Signal));
    let deferred = count(&|s, _| s == Shape::Deferred);
    let rich = count(&|_, m| m == Mode::Rich);
    let all: String = rewritten.values().map(String::as_str).collect();
    assert_eq!(all.matches("Signal::derive(move || tr!(").count(), signal);
    assert_eq!(all.matches("|| tr!(").count() - signal, deferred);
    assert_eq!(all.matches(".before\")").count(), rich);
    eprintln!(
        "{} sites: {signal} with an argument read from a signal, {deferred} deferred, {rich} split sentences",
        plan.sites.len()
    );
}

/// Negative control: with `move-tr-view` off, its sites are reported, not
/// rewritten, and the comparison fails.
#[test]
fn a_rule_disabled_leaves_its_sites_reported_and_the_comparison_fails() {
    let files = generated();
    let (rewritten, report) = rewritten(&files, &options(Some(Rule::MoveTrView)));
    let reported = report
        .findings
        .iter()
        .filter(|f| f.code == Code::LfCall && f.message.contains("`move-tr-view`"))
        .count();
    assert!(reported > 100, "{}", report.to_text());
    assert!(!differing(&files, &rewritten).is_empty());
}

/// Every rule of §6.2 has its own test, named after it, and the plan names
/// every rule.
#[test]
fn every_rule_is_tested_and_planned() {
    let tests = include_str!("../../../tests/convert.rs");
    let plan = include_str!("../../../../../plans/05-tooling.md");
    for rule in Rule::ALL {
        let name = rule.name();
        assert!(
            tests.contains(&format!("fn rule_{}(", name.replace('-', "_"))),
            "no test named rule_{name} in tests/convert.rs"
        );
        assert!(
            plan.contains(&format!("| `{name}` |")),
            "{name} is not in plans/05 §6.2"
        );
    }
}
