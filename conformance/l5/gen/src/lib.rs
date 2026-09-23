//! Builds one conformance L5 crate (`plans/01-conformance.md` §3,
//! `plans/13-phase-5b-work-order.md` A4).
//!
//! L5 is the suite through `tr!`. That needs a real i18n crate per locale the
//! suite uses — a corpus `mf2-build` compiles, a manifest, a generated module
//! and one call site per test — and it needs them to be *the suite*, not a
//! copy of it. So an L5 crate's build script calls [`generate`], which reads
//! the vendored suite directly and writes, into `OUT_DIR`:
//!
//! * `locales/<tag>.json` — every runtime-valid test of that locale, keyed
//!   `suite.<file>.t<index>` (the flat JSON loader, which can carry any
//!   source; the container syntax could not write the malformed ones);
//! * the catalogs, the manifest and the generated module, from a real
//!   `mf2_build::Build`;
//! * `cases.rs` — one call site per test: `tr!` when the test's `params` are
//!   exactly the message's variables, `tr_dyn` when they deliberately are
//!   not (the ledger's `via = "dyn"`), and a table of the ids the build
//!   **refused**, with the MF2 error kinds it refused them with — which is
//!   what a syntax- or data-model-error test asserts at L5.
//!
//! Nothing is checked in, so nothing drifts: `cargo xtask spec-sync` is
//! carried through by the next build.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mf2_build::loader::json;
use mf2_build::{Build, Config, Features, Level, Lint};
use serde_json::Value;

/// The suite's data-model errors, by the names it gives them
/// (`plans/01-conformance.md` §2). Duplicated from `mf2-conformance` on
/// purpose: this crate is a **build** dependency of the L5 crates, and
/// `mf2-conformance` depends on those, so it cannot be depended on back.
const DATA_MODEL_ERRORS: [&str; 6] = [
    "duplicate-declaration",
    "duplicate-option-name",
    "duplicate-variant",
    "missing-fallback-variant",
    "missing-selector-annotation",
    "variant-key-mismatch",
];

/// The functions the suite defines for itself (`test/README.md`), which the
/// generated registry reaches through `mf2-l4-runner` — the same handlers
/// L4 formats with, through the public custom-function API and nowhere else.
const TEST_FUNCTIONS: [(&str, &str); 3] = [
    ("test:format", "::mf2_l4_runner::test_functions::FORMAT"),
    ("test:function", "::mf2_l4_runner::test_functions::FUNCTION"),
    ("test:select", "::mf2_l4_runner::test_functions::SELECT"),
];

/// Anything that stopped the generation.
pub type Error = Box<dyn std::error::Error>;

/// One of the suite's tests, as this crate needs it.
struct Test {
    id: String,
    src: String,
    /// The `params`, in the order written.
    params: Vec<(String, Param)>,
    /// Whether the spec refuses the message (so it cannot be in a corpus).
    invalid: bool,
}

/// A `params` value, as the call site will pass it.
enum Param {
    Str(String),
    Int(i64),
    Float(f64),
    /// A typed `datetime` parameter: its literal, parsed at run time.
    DateTime(String),
    /// A boolean or anything else no function takes: an application value
    /// with no conversions, as L4's runner passes it.
    Opaque,
}

impl Param {
    /// The Rust expression for it, at a `tr!` call site.
    fn expr(&self) -> String {
        match self {
            Param::Str(s) => rust_str(s),
            Param::Int(n) => format!("{n}i64"),
            Param::Float(x) => format!("{x:?}f64"),
            Param::DateTime(s) => format!("date_time({})", rust_str(s)),
            Param::Opaque => "opaque()".to_owned(),
        }
    }
}

/// A Rust string literal for `s`.
fn rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Builds the L5 crate for `locale`, from the repository at `root`, into
/// `out` (`OUT_DIR`). Call it from an L5 crate's `build.rs`.
pub fn generate(root: &Path, out: &Path, locale: &str) -> Result<(), Error> {
    let suite_dir = root.join("third_party/message-format-wg/test/tests");
    let extra_dir = root.join("conformance/extra");
    println!("cargo::rerun-if-changed={}", suite_dir.display());
    println!("cargo::rerun-if-changed={}", extra_dir.display());

    let mut tests = Vec::new();
    collect(&suite_dir, &suite_dir, "suite", locale, &mut tests)?;
    if extra_dir.is_dir() {
        collect(&extra_dir, &extra_dir, "suite.extra", locale, &mut tests)?;
    }
    tests.sort_by(|a, b| a.id.cmp(&b.id));

    let (invalid, valid): (Vec<&Test>, Vec<&Test>) = tests.iter().partition(|t| t.invalid);
    let manifest = build_corpus(out, locale, &valid)?;
    let rejected = build_rejected(out, locale, &invalid)?;
    let gated = build_gated(out, locale)?;
    write_cases(out, locale, &valid, &manifest, &rejected, &gated)?;
    Ok(())
}

/// Reads every `*.json` under `dir`, applying `defaultTestProperties`, and
/// keeps the tests of `locale`.
fn collect(
    base: &Path,
    dir: &Path,
    prefix: &str,
    locale: &str,
    out: &mut Vec<Test>,
) -> Result<(), Error> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(base, &path, prefix, locale, out)?;
            continue;
        }
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: Value = serde_json::from_str(&text)?;
        let Some(tests) = file.get("tests").and_then(Value::as_array) else {
            continue;
        };
        let defaults = file.get("defaultTestProperties").and_then(Value::as_object);
        // `suite.functions.number` — the file's path, so that a test of
        // `extra/functions/…` can never collide with one of `functions/…`.
        let stem = path
            .strip_prefix(base)?
            .with_extension("")
            .to_string_lossy()
            .replace(['/', '\\'], ".");
        for (index, test) in tests.iter().enumerate() {
            let mut resolved = defaults.cloned().unwrap_or_default();
            if let Some(object) = test.as_object() {
                for (k, v) in object {
                    resolved.insert(k.clone(), v.clone());
                }
            }
            if resolved.get("locale").and_then(Value::as_str).unwrap_or("") != locale {
                continue;
            }
            let Some(src) = resolved.get("src").and_then(Value::as_str) else {
                continue;
            };
            let errors: Vec<&str> = resolved
                .get("expErrors")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.get("type").and_then(Value::as_str))
                        .collect()
                })
                .unwrap_or_default();
            let invalid = errors
                .iter()
                .any(|e| *e == "syntax-error" || DATA_MODEL_ERRORS.contains(e));
            out.push(Test {
                id: format!("{prefix}.{stem}.t{index:03}"),
                src: src.to_owned(),
                params: params(&resolved),
                invalid,
            });
        }
    }
    Ok(())
}

/// The test's `params`, as the call site will pass them.
fn params(resolved: &serde_json::Map<String, Value>) -> Vec<(String, Param)> {
    let Some(list) = resolved.get("params").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_owned();
            let typed = p.get("type").and_then(Value::as_str);
            let value = match (typed, p.get("value")) {
                (Some("datetime"), Some(Value::String(s))) => Param::DateTime(s.clone()),
                (None, Some(Value::String(s))) => Param::Str(s.clone()),
                (None, Some(Value::Number(n))) => match n.as_i64() {
                    Some(i) => Param::Int(i),
                    None => n.as_f64().map_or(Param::Opaque, Param::Float),
                },
                _ => Param::Opaque,
            };
            Some((name, value))
        })
        .collect()
}

/// The corpus configuration: the suite exercises the *runtime's* errors on
/// purpose — functions nothing provides, a `select` from a variable, option
/// values an option cannot take, lone markup — which a real corpus refuses
/// as a matter of policy (`plans/05-tooling.md` §5). Policy is not what L5
/// is about, so every lint goes to its floor.
fn config(locale: &str) -> Config {
    let mut config = Config {
        source_locale: locale.to_owned(),
        ..Config::default()
    };
    for &lint in Lint::ALL {
        let floor = lint.floor();
        if floor != Level::Error {
            config.lints.insert(lint, floor);
        }
    }
    for (name, path) in TEST_FUNCTIONS {
        config.functions.insert(name.to_owned(), path.to_owned());
    }
    config
}

/// Every feature, so that the catalogs carry what layer L4 formats with; the
/// default configuration (L5d) is the same catalogs with the default
/// registry, exactly as L4d is.
fn features() -> Features {
    Features::parse("fn-number,fn-datetime,datetime-icu")
}

/// Writes the corpus and builds it: catalogs, manifest and the generated
/// module, into `out`.
fn build_corpus(out: &Path, locale: &str, tests: &[&Test]) -> Result<mf2_build::Manifest, Error> {
    let root = out.join("corpus");
    let locales = root.join("locales");
    std::fs::create_dir_all(&locales)?;
    let records: Vec<(&str, &str)> = tests
        .iter()
        .map(|t| (t.id.as_str(), t.src.as_str()))
        .collect();
    std::fs::write(
        locales.join(format!("{locale}.json")),
        json::write(records.iter().copied()),
    )?;
    let outcome = Build::at(&root, out)
        .config(config(locale))
        .features(features())
        .run()?;
    let outcome = outcome.into_result().map_err(|e| {
        format!("the suite's own messages did not build as a corpus for {locale}: {e}")
    })?;
    Ok(outcome.manifest)
}

/// Builds the messages the spec refuses, and collects what the build said
/// about each: the MF2 error kinds, by the suite's names for them.
fn build_rejected(
    out: &Path,
    locale: &str,
    tests: &[&Test],
) -> Result<BTreeMap<String, Vec<String>>, Error> {
    let mut kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if tests.is_empty() {
        return Ok(kinds);
    }
    let root = out.join("refused");
    let locales = root.join("locales");
    std::fs::create_dir_all(&locales)?;
    let records: Vec<(&str, &str)> = tests
        .iter()
        .map(|t| (t.id.as_str(), t.src.as_str()))
        .collect();
    std::fs::write(
        locales.join(format!("{locale}.json")),
        json::write(records.iter().copied()),
    )?;
    // `check`: the build writes nothing, and its report is the verdict.
    let outcome = Build::at(&root, out.join("refused-out"))
        .config(config(locale))
        .features(features())
        .check()?;
    for diagnostic in &outcome.report.diagnostics {
        let (Some(id), Some(kind)) = (&diagnostic.id, diagnostic.kind) else {
            continue;
        };
        let names = kinds.entry(id.clone()).or_default();
        let name = kind.suite_name().to_owned();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for names in kinds.values_mut() {
        names.sort();
    }
    Ok(kinds)
}

/// The same corpus in the **default** configuration (layer L5d): which of
/// its messages the build refuses because they name a function whose client
/// feature is off, and what it says about each. This is the build's verdict
/// standing where L4d has a run-time *Unknown Function* — a translation can
/// never add formatting code to the wasm by itself, so the build stops
/// first (`plans/05-tooling.md` §5).
fn build_gated(out: &Path, locale: &str) -> Result<BTreeMap<String, String>, Error> {
    let root = out.join("corpus");
    let outcome = Build::at(&root, out.join("default-out"))
        .config(config(locale))
        .features(Features::default())
        .check()?;
    let mut gated = BTreeMap::new();
    for diagnostic in &outcome.report.diagnostics {
        if diagnostic.lint != Some(Lint::GatedFunction) || diagnostic.level != Level::Error {
            continue;
        }
        if let Some(id) = &diagnostic.id {
            gated.entry(id.clone()).or_insert_with(|| {
                format!(
                    "{}:{}:{}: {}",
                    diagnostic
                        .file
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                    diagnostic.line,
                    diagnostic.column,
                    diagnostic.message
                )
            });
        }
    }
    Ok(gated)
}

/// Writes `cases.rs`: one call site per runtime-valid test, and the tables of
/// what the build refused, in both configurations.
fn write_cases(
    out: &Path,
    locale: &str,
    tests: &[&Test],
    manifest: &mf2_build::Manifest,
    rejected: &BTreeMap<String, Vec<String>>,
    gated: &BTreeMap<String, String>,
) -> Result<(), Error> {
    let mut s = String::with_capacity(64 * 1024);
    let _ = writeln!(
        s,
        "// @generated by mf2-l5-gen from the vendored suite — do not edit.\n\
         // {} call sites, {} messages the build refused; locale {locale}.\n",
        tests.len(),
        rejected.len()
    );
    let _ = writeln!(
        s,
        "/// Every test of this locale whose message the build accepted, as the\n\
         /// call site that formats it.\n\
         pub static CASES: &[Site] = &["
    );
    let mut dynamic: Vec<&str> = Vec::new();
    for test in tests {
        let slots = manifest
            .msg_id(&test.id)
            .map(|id| id.index() as usize)
            .and_then(|i| manifest.slots.get(i))
            .map_or(&[][..], Vec::as_slice);
        let names: Vec<&str> = test.params.iter().map(|(n, _)| n.as_str()).collect();
        // `tr!` when the call site can name exactly the message's variables;
        // otherwise the test is one of the suite's deliberate mismatches and
        // goes through the dynamic path (`via = "dyn"`).
        let positional = slots.len() == names.len()
            && slots.iter().all(|slot| names.contains(&slot.as_str()))
            && names.len()
                == names
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len();
        let id = rust_str(&test.id);
        if positional {
            let mut args = String::new();
            for (name, value) in &test.params {
                let _ = write!(args, ", {} = {}", rust_str(name), value.expr());
            }
            let case = if test.params.is_empty() {
                format!("Case::Tr(tr!({id}))")
            } else {
                format!("Case::Args(tr!({id}{args}))")
            };
            let _ = writeln!(s, "    ({id}, || {case}),");
        } else {
            dynamic.push(&test.id);
            let mut args = String::new();
            for (name, value) in &test.params {
                let _ = write!(
                    args,
                    "(::mf2::Text::Static({}), ::mf2::ArgValue::from({})), ",
                    rust_str(name),
                    value.expr()
                );
            }
            let _ = writeln!(
                s,
                "    ({id}, || Case::Dyn(::mf2::tr_dyn(msg_id!({id}), \
                 ::std::vec![{args}]))),"
            );
        }
    }
    let _ = writeln!(s, "];\n");

    let _ = writeln!(
        s,
        "/// The tests whose `params` do not name the message's variables, so\n\
         /// the call site passes them by name (the ledger's `via = \"dyn\"`).\n\
         pub static VIA_DYN: &[&str] = &["
    );
    for id in &dynamic {
        let _ = writeln!(s, "    {},", rust_str(id));
    }
    let _ = writeln!(s, "];\n");

    let _ = writeln!(
        s,
        "/// What the build refused, and the MF2 error kinds it refused it\n\
         /// with — a syntax- or data-model-error test's whole assertion at L5.\n\
         pub static REJECTED: &[(&str, &[&str])] = &["
    );
    for (id, kinds) in rejected {
        let list: Vec<String> = kinds.iter().map(|k| rust_str(k)).collect();
        let _ = writeln!(s, "    ({}, &[{}]),", rust_str(id), list.join(", "));
    }
    let _ = writeln!(s, "];\n");

    let _ = writeln!(
        s,
        "/// What the build refuses in the **default** configuration (L5d):\n\
         /// the messages that name a function whose client feature is off, with\n\
         /// the error, its file and its line — the build's verdict where L4d has\n\
         /// a run-time Unknown Function.\n\
         pub static GATED: &[(&str, &str)] = &["
    );
    for (id, detail) in gated {
        let _ = writeln!(s, "    ({}, {}),", rust_str(id), rust_str(detail));
    }
    let _ = writeln!(s, "];");

    let path = out.join("cases.rs");
    if std::fs::read_to_string(&path).is_ok_and(|old| old == s) {
        return Ok(());
    }
    std::fs::write(&path, s)?;
    Ok(())
}
