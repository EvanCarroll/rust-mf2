//! `mf2 convert --from fluent` (Phase 8 A1; the tooling design §6.1),
//! run as a user runs it.
//!
//! The construct corpus under `tests/fluent/constructs/` holds every entry
//! and expression kind of `fluent-syntax`'s AST; its expected output is
//! `tests/fluent/constructs.expected/`, compared in the crate's own tests
//! (`src/convert.rs`, with the negative control). Here: the command end to
//! end, and one test per code, named after it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn mf2() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mf2"))
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(mf2())
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("the mf2 binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A fresh directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("convert")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// Writes `files` (`locale/path.ftl` → text) under `<dir>/ftl`.
fn ftl(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
    let root = dir.join("ftl");
    for (path, body) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }
    root
}

/// What `mf2 convert --format json` reported, and whether it exited 0.
struct Converted {
    ok: bool,
    diagnostics: Vec<Value>,
    out: PathBuf,
}

impl Converted {
    fn one(&self, code: &str) -> &Value {
        let found: Vec<&Value> = self
            .diagnostics
            .iter()
            .filter(|d| d["code"] == code)
            .collect();
        assert_eq!(found.len(), 1, "{code}: {:#?}", self.diagnostics);
        found[0]
    }

    fn file(&self, rel: &str) -> String {
        std::fs::read_to_string(self.out.join("locales").join(rel)).unwrap_or_default()
    }
}

fn convert(name: &str, files: &[(&str, &str)]) -> Converted {
    let dir = scratch(name);
    let input = ftl(&dir, files);
    let out = dir.join("out");
    let output = run(
        &out,
        &[
            "convert",
            "--from",
            "fluent",
            input.to_str().expect("utf-8"),
            "--format",
            "json",
        ],
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON on stdout: {e}\n{}\n{}",
            text(&output.stdout),
            text(&output.stderr)
        )
    });
    Converted {
        ok: output.status.success(),
        diagnostics: json["diagnostics"].as_array().cloned().unwrap_or_default(),
        out,
    }
}

/// Asserts a finding's level, place and entry.
fn at(d: &Value, level: &str, line: u64, column: u64, id: Option<&str>) {
    assert_eq!(d["level"], level, "{d:#}");
    assert_eq!(
        (d["line"].as_u64(), d["column"].as_u64()),
        (Some(line), Some(column)),
        "{d:#}"
    );
    assert_eq!(d["id"].as_str(), id, "{d:#}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir").flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copy");
        }
    }
}

#[test]
fn the_construct_corpus_converts_checks_and_is_canonical() {
    let dir = scratch("constructs");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs");
    let output = run(
        &dir,
        &[
            "convert",
            "--from",
            "fluent",
            corpus.to_str().expect("utf-8"),
        ],
    );
    assert!(
        output.status.success(),
        "{}{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    let report = text(&output.stdout);
    assert!(report.contains("0 error(s), 7 warning(s)"), "{report}");
    assert!(
        report.contains("note: the output needs the client feature(s) fn-datetime, fn-number"),
        "{report}"
    );

    let expected = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs.expected");
    for locale in std::fs::read_dir(&expected).expect("expected").flatten() {
        for file in std::fs::read_dir(locale.path())
            .expect("a locale")
            .flatten()
        {
            let rel = file
                .path()
                .strip_prefix(&expected)
                .expect("under")
                .to_path_buf();
            assert_eq!(
                std::fs::read_to_string(dir.join("locales").join(&rel)).ok(),
                std::fs::read_to_string(file.path()).ok(),
                "{}",
                rel.display()
            );
        }
    }

    // What it writes is a corpus `mf2 check` accepts and `mf2 fmt` would
    // not change.
    std::fs::write(dir.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
    let check = run(&dir, &["check", "--features", "fn-number,fn-datetime"]);
    assert!(check.status.success(), "{}", text(&check.stdout));
    assert!(
        text(&check.stdout).contains("0 error(s)")
            || text(&check.stdout).contains("nothing to report")
    );
    let fmt = run(&dir, &["fmt", "--check"]);
    assert!(fmt.status.success(), "{}", text(&fmt.stdout));

    // A second run finds every file as it would write it: it writes
    // nothing and exits as the first did.
    let written = report
        .lines()
        .last()
        .and_then(|l| l.split(" file(s) written").next())
        .and_then(|l| l.rsplit(' ').next())
        .expect("the summary")
        .to_owned();
    let again = run(
        &dir,
        &[
            "convert",
            "--from",
            "fluent",
            corpus.to_str().expect("utf-8"),
        ],
    );
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert!(
        text(&again.stdout).contains(&format!("0 file(s) written ({written} unchanged)")),
        "{}",
        text(&again.stdout)
    );

    // One that would overwrite other text stops before writing anything.
    let before = std::fs::read_to_string(dir.join("locales/en/selects.mf2")).expect("written");
    std::fs::write(dir.join("locales/en/selects.mf2"), "edited").expect("write");
    let again = run(
        &dir,
        &[
            "convert",
            "--from",
            "fluent",
            corpus.to_str().expect("utf-8"),
        ],
    );
    assert!(!again.status.success());
    assert!(
        text(&again.stderr).contains("already exists"),
        "{}",
        text(&again.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("locales/en/selects.mf2")).expect("still there"),
        "edited"
    );
    assert_ne!(before, "edited");
}

/// Phase 8 A2's corpus: the reference workload as Fluent, written by
/// `workload-gen --format ftl`, every locale.
#[test]
fn the_reference_workload_converts_with_nothing_unmapped() {
    use workload_gen::{Format, Knobs, Workload, fluent, locale};

    let dir = scratch("workload");
    let knobs = Knobs::default();
    let files = workload_gen::generate_as(&knobs, &[Format::Ftl], &[]).expect("the workload");
    for (path, bytes) in files.iter() {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, bytes).expect("write");
    }
    let out = dir.join("out");
    let output = run(
        &out,
        &[
            "convert",
            "--from",
            "fluent",
            dir.join("ftl").to_str().expect("utf-8"),
            "--format",
            "json",
        ],
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("a JSON report");
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert_eq!(json["diagnostics"], Value::Array(Vec::new()), "{json:#}");
    // 1,600 messages, of which 8 are sentences split around an element, each
    // into three.
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        assert_eq!(json["entries"][tag], 1600 - 8 + 3 * 8, "{tag}");
    }

    std::fs::write(out.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
    let check = run(&out, &["check"]);
    assert!(
        text(&check.stdout).contains(" 0 error(s)"),
        "{}",
        text(&check.stdout)
    );
    assert!(run(&out, &["fmt", "--check"]).status.success());

    // Every message says what the `.mf2` original says: the same source text
    // under the Fluent id, except that a plural select declares `:number`
    // (Fluent has no integer) and a split sentence is its three parts.
    let wl = Workload::generate(&knobs).expect("the plan");
    let ids = fluent::ids(&wl).expect("the ids");
    for loc in locale::locales(&knobs).expect("the locales") {
        let exported = run(&out, &["export", loc.tag]);
        assert!(exported.status.success(), "{}", text(&exported.stderr));
        let converted: Value = serde_json::from_slice(&exported.stdout).expect("flat JSON");
        let original = locale::sources(&wl, &loc);
        let (mut same, mut selects, mut split) = (0, 0, 0);
        for (j, message) in wl.messages.iter().enumerate() {
            let what = format!("{} {}", loc.tag, message.id);
            if !message.markup.is_empty() {
                for part in ["before", message.markup[0], "after"] {
                    let id = format!("{}.{part}", ids[j]);
                    assert!(converted[&id].is_string(), "{what}: {id}");
                }
                split += 1;
            } else if message.is_select() {
                let want = original[j].replacen(" :integer}", " :number}", 1);
                assert_eq!(converted[&ids[j]].as_str(), Some(want.as_str()), "{what}");
                selects += 1;
            } else {
                assert_eq!(
                    converted[&ids[j]].as_str(),
                    Some(original[j].as_str()),
                    "{what}"
                );
                same += 1;
            }
        }
        assert_eq!((same, selects, split), (1578, 14, 8), "{}", loc.tag);
    }
}

#[test]
fn errors_leave_the_entry_out_and_the_rest_is_written() {
    let c = convert(
        "partial",
        &[(
            "en/main.ftl",
            "good = Fine\nbad = {missing}\nalso-good = Also fine\n",
        )],
    );
    assert!(!c.ok);
    let main = c.file("en/main.mf2");
    assert!(main.contains("good = Fine"), "{main}");
    assert!(main.contains("also-good = Also fine"), "{main}");
    assert!(!main.contains("bad"), "{main}");
}

#[test]
fn a_locale_in_a_copied_tree_keeps_its_files_apart() {
    // Two locales, the same file names: each goes to its own directory.
    let dir = scratch("two-locales");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs");
    copy_dir(&corpus.join("fr"), &dir.join("ftl/de"));
    copy_dir(&corpus.join("fr"), &dir.join("ftl/fr"));
    let out = dir.join("out");
    let output = run(
        &out,
        &[
            "convert",
            "--from",
            "fluent",
            dir.join("ftl").to_str().expect("utf-8"),
        ],
    );
    assert!(output.status.success(), "{}", text(&output.stdout));
    let de = std::fs::read_to_string(out.join("locales/de/selects.mf2")).expect("de");
    assert!(de.starts_with("@locale de\n"), "{de}");
    assert!(out.join("locales/fr/selects.mf2").is_file());
}

#[test]
fn a_tab_in_a_comment_does_not_cost_the_file() {
    let c = convert(
        "comment-tab",
        &[("en/main.ftl", "# Column\tlayout\nm = Text\t with a tab\n")],
    );
    assert!(c.ok, "{:#?}", c.diagnostics);
    let main = c.file("en/main.mf2");
    assert!(main.contains("# Column layout\n"), "{main}");
    assert!(main.contains("m = Text\\t with a tab"), "{main}");
}

// One test per code (§6.1, "The codes"), named after it.

#[test]
fn fluent_junk() {
    let c = convert("junk", &[("en/main.ftl", "ok = Fine\nbroken = {\n")]);
    assert!(!c.ok);
    // At the parser's own position: where it expected an expression.
    at(c.one("fluent-junk"), "error", 3, 1, None);
    assert!(c.file("en/main.mf2").contains("ok = Fine"));
}

#[test]
fn fluent_missing_reference() {
    let c = convert(
        "missing",
        &[("en/main.ftl", "msg = Before {-nowhere} after\n")],
    );
    assert!(!c.ok);
    at(
        c.one("fluent-missing-reference"),
        "error",
        1,
        16,
        Some("msg"),
    );
    assert!(!c.file("en/main.mf2").contains("msg"));

    let c = convert(
        "missing-attribute",
        &[("en/main.ftl", "a = A\n    .x = X\nb = {a.y}\n")],
    );
    at(c.one("fluent-missing-reference"), "error", 3, 6, Some("b"));
}

#[test]
fn fluent_cyclic_reference() {
    let c = convert("cycle", &[("en/main.ftl", "a = A {b}\nb = B {a}\n")]);
    assert!(!c.ok);
    // Converting `a` meets `a` again inside `b`, and converting `b` meets
    // `b` inside `a`; findings are sorted by position.
    let cycles: Vec<&Value> = c
        .diagnostics
        .iter()
        .filter(|d| d["code"] == "fluent-cyclic-reference")
        .collect();
    assert_eq!(cycles.len(), 2, "{:#?}", c.diagnostics);
    at(cycles[0], "error", 1, 8, Some("b"));
    at(cycles[1], "error", 2, 8, Some("a"));
}

#[test]
fn fluent_number_operand() {
    let c = convert(
        "number-operand",
        &[("en/main.ftl", "n = {NUMBER(\"five\")}\n")],
    );
    assert!(!c.ok);
    at(c.one("fluent-number-operand"), "error", 1, 6, Some("n"));
}

#[test]
fn fluent_currency_missing() {
    let c = convert(
        "currency",
        &[("en/main.ftl", "price = {NUMBER($p, style: \"currency\")}\n")],
    );
    assert!(!c.ok);
    at(
        c.one("fluent-currency-missing"),
        "error",
        1,
        10,
        Some("price"),
    );
}

#[test]
fn fluent_datetime_option() {
    let c = convert(
        "datetime-option",
        &[("en/main.ftl", "d = {DATETIME($d, era: \"long\")}\n")],
    );
    assert!(!c.ok);
    at(c.one("fluent-datetime-option"), "error", 1, 6, Some("d"));
}

#[test]
fn fluent_date_selector() {
    let c = convert(
        "date-selector",
        &[("en/main.ftl", "d = {DATETIME($d) ->\n   *[other] x\n}\n")],
    );
    assert!(!c.ok);
    at(c.one("fluent-date-selector"), "error", 1, 6, Some("d"));
}

#[test]
fn fluent_unknown_function() {
    let c = convert("unknown-function", &[("en/main.ftl", "p = {PLATFORM()}\n")]);
    assert!(!c.ok);
    at(c.one("fluent-unknown-function"), "error", 1, 6, Some("p"));
}

#[test]
fn fluent_mixed_keys() {
    let c = convert(
        "mixed-keys",
        &[(
            "en/main.ftl",
            "m = {$x ->\n    [one] a\n    [male] b\n   *[other] c\n}\n",
        )],
    );
    assert!(!c.ok);
    at(c.one("fluent-mixed-keys"), "error", 2, 6, Some("m"));
}

#[test]
fn fluent_variant_limit() {
    // Nine independent two-way selects: 512 variants.
    let mut body = String::from("big =");
    for i in 0..9 {
        let _ = write!(body, " {{$v{i} ->\n    [a] a\n   *[b] b\n}}");
    }
    body.push('\n');
    let c = convert("variant-limit", &[("en/main.ftl", &body)]);
    assert!(!c.ok);
    at(c.one("fluent-variant-limit"), "error", 1, 1, Some("big"));
}

#[test]
fn fluent_file_collision() {
    let c = convert(
        "file-collision",
        &[("en/a/b.ftl", "one = One\n"), ("en/a.b.ftl", "two = Two\n")],
    );
    assert!(!c.ok);
    // Path order is by component: `a/b.ftl` comes first and is kept.
    let d = c.one("fluent-file-collision");
    at(d, "error", 1, 1, None);
    assert!(
        d["file"]
            .as_str()
            .is_some_and(|f| f.ends_with("en/a.b.ftl")),
        "{d:#}"
    );
    assert!(c.file("en/a.b.mf2").contains("one = One"));
}

#[test]
fn fluent_duplicate_id() {
    let c = convert(
        "duplicate-id",
        &[
            ("en/a.ftl", "same = First\n"),
            ("en/b.ftl", "other = Other\nsame = Second\n"),
        ],
    );
    assert!(!c.ok);
    at(c.one("fluent-duplicate-id"), "error", 2, 1, Some("same"));
    assert!(c.file("en/a.mf2").contains("same = First"));
    assert!(!c.file("en/b.mf2").contains("Second"));
}

#[test]
fn fluent_locale() {
    let c = convert(
        "locale",
        &[("en/main.ftl", "a = A\n"), ("12/main.ftl", "a = A\n")],
    );
    assert!(!c.ok);
    let d = c.one("fluent-locale");
    at(d, "error", 1, 1, None);
    assert_eq!(d["locale"], "12");
    assert!(c.file("en/main.mf2").contains("a = A"));
}

#[test]
fn fluent_unbound_term_variable() {
    // The term lives in another file, which the finding names.
    let c = convert(
        "unbound",
        &[
            ("en/terms.ftl", "-t = Hi {$who}\n"),
            ("en/main.ftl", "m = {-t}\n"),
        ],
    );
    assert!(c.ok);
    let d = c.one("fluent-unbound-term-variable");
    at(d, "warn", 1, 11, Some("m"));
    assert!(
        d["file"].as_str().is_some_and(|f| f.ends_with("terms.ftl")),
        "{d:#}"
    );
    // Faithful: `fluent-bundle` writes the reference as text.
    assert!(
        c.file("en/main.mf2").contains(r"m = Hi \{$who\}"),
        "{}",
        c.file("en/main.mf2")
    );
}

#[test]
fn fluent_term_positional() {
    let c = convert(
        "positional",
        &[("en/main.ftl", "-t = Term\nm = {-t(\"x\")}\n")],
    );
    assert!(c.ok);
    at(c.one("fluent-term-positional"), "warn", 2, 7, Some("m"));
    assert!(c.file("en/main.mf2").contains("m = Term"));
}

#[test]
fn fluent_number_option() {
    let c = convert(
        "number-option",
        &[(
            "en/main.ftl",
            "n = {NUMBER($n, minimumFractionDigits: \"2\", bogus: 1)}\n",
        )],
    );
    assert!(c.ok);
    let found: Vec<&Value> = c
        .diagnostics
        .iter()
        .filter(|d| d["code"] == "fluent-number-option")
        .collect();
    assert_eq!(found.len(), 2, "{:#?}", c.diagnostics);
    at(found[0], "warn", 1, 17, Some("n"));
    at(found[1], "warn", 1, 45, Some("n"));
    assert!(c.file("en/main.mf2").contains("n = {$n :number}"));
}

#[test]
fn fluent_unreachable_variant() {
    // `[1]` after `[one]` in English: `fluent-bundle` takes `one` first.
    let c = convert(
        "unreachable",
        &[(
            "en/main.ftl",
            "m = {$n ->\n    [one] one\n    [1] never\n   *[other] other\n}\n",
        )],
    );
    assert!(c.ok);
    at(c.one("fluent-unreachable-variant"), "warn", 3, 6, Some("m"));
    assert!(!c.file("en/main.mf2").contains("never"));
}

#[test]
fn fluent_datetime_approximate() {
    let c = convert(
        "datetime-approximate",
        &[("en/main.ftl", "d = {DATETIME($d, dateStyle: \"short\")}\n")],
    );
    assert!(c.ok);
    at(
        c.one("fluent-datetime-approximate"),
        "warn",
        1,
        6,
        Some("d"),
    );
    assert!(
        c.file("en/main.mf2")
            .contains("d = {$d :date length=short}")
    );
}

// `mf2 convert --from leptos-fluent` (Phase 8 A4; the tooling design §6.2): one test
// per rule and one per code, on a one-file application.

/// The messages every application below has.
const APP_FTL: &str = "hello = Hello
greet = Hello, { $name }!
count = { $n ->
    [one] One item
   *[other] { $n } items
}
two = { $a-b } and { $c }
hint =
    .before = Press
";

/// What `mf2 convert --from leptos-fluent --write --format json` did to an
/// application whose only source file is `src/lib.rs`.
struct Migrated {
    ok: bool,
    diagnostics: Vec<Value>,
    app: PathBuf,
    lib: String,
}

impl Migrated {
    fn one(&self, code: &str) -> &Value {
        let found: Vec<&Value> = self
            .diagnostics
            .iter()
            .filter(|d| d["code"] == code)
            .collect();
        assert_eq!(found.len(), 1, "{code}: {:#?}", self.diagnostics);
        found[0]
    }

    /// Asserts that nothing was reported.
    fn clean(&self) -> &Self {
        assert!(
            self.ok && self.diagnostics.is_empty(),
            "{:#?}",
            self.diagnostics
        );
        self
    }
}

/// Writes an application (`Cargo.toml`, `locales/en/main.ftl`, `src/lib.rs`)
/// and migrates it.
fn migrate_with(name: &str, manifest: &str, lib: &str, extra: &[&str]) -> Migrated {
    let dir = scratch(&format!("lf-{name}"));
    let app = dir.join("app");
    for (path, body) in [
        ("Cargo.toml", manifest),
        ("locales/en/main.ftl", APP_FTL),
        ("src/lib.rs", lib),
    ] {
        let full = app.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }
    let mut args = vec![
        "convert",
        "--from",
        "leptos-fluent",
        app.to_str().expect("utf-8"),
        "--i18n-crate",
        "app-i18n",
        "--write",
        "--format",
        "json",
    ];
    args.extend_from_slice(extra);
    let output = run(&dir.join("i18n"), &args);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON on stdout: {e}\n{}\n{}",
            text(&output.stdout),
            text(&output.stderr)
        )
    });
    Migrated {
        ok: output.status.success(),
        diagnostics: json["diagnostics"].as_array().cloned().unwrap_or_default(),
        lib: std::fs::read_to_string(app.join("src/lib.rs")).expect("the source"),
        app,
    }
}

fn migrate(name: &str, lib: &str) -> Migrated {
    migrate_with(name, "[package]\nname = \"app\"\n", lib, &[])
}

#[test]
fn a_leptos_fluent_application_is_converted_and_its_calls_rewritten() {
    let m = migrate(
        "whole",
        "use leptos_fluent::{move_tr, tr};\n\nfn f() -> String {\n    tr!(\"hello\")\n}\n",
    );
    m.clean();
    assert_eq!(
        m.lib,
        "use app_i18n::tr;\n\nfn f() -> String {\n    tr!(\"hello\").to_string()\n}\n"
    );
    let main = m
        .app
        .parent()
        .expect("dir")
        .join("i18n/locales/en/main.mf2");
    assert!(
        std::fs::read_to_string(main)
            .expect("converted")
            .contains("hello = Hello")
    );
}

#[test]
fn without_write_nothing_is_written_and_the_diff_is_shown() {
    let dir = scratch("lf-dry");
    let app = dir.join("app");
    std::fs::create_dir_all(app.join("src")).expect("mkdir");
    std::fs::create_dir_all(app.join("locales/en")).expect("mkdir");
    std::fs::write(app.join("Cargo.toml"), "[package]\nname = \"app\"\n").expect("write");
    std::fs::write(app.join("locales/en/main.ftl"), APP_FTL).expect("write");
    let lib = "use leptos_fluent::tr;\nfn f() -> String { tr!(\"hello\") }\n";
    std::fs::write(app.join("src/lib.rs"), lib).expect("write");
    let out = run(
        &dir.join("i18n"),
        &[
            "convert",
            "--from",
            "leptos-fluent",
            app.to_str().expect("utf-8"),
            "--i18n-crate",
            "app_i18n",
        ],
    );
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("--- a/src/lib.rs\n+++ b/src/lib.rs\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("+fn f() -> String { tr!(\"hello\").to_string() }"),
        "{stdout}"
    );
    assert!(stdout.contains("would write "), "{stdout}");
    assert_eq!(
        std::fs::read_to_string(app.join("src/lib.rs")).expect("read"),
        lib
    );
    assert!(!dir.join("i18n/locales").exists());
}

#[test]
fn a_second_run_changes_nothing() {
    let m = migrate(
        "again",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"hello\") }\n",
    );
    m.clean();
    let dir = m.app.parent().expect("dir").to_path_buf();
    let before = tree(&dir);

    // `--write` again: it exits as the first did and writes nothing.
    let out = convert_app(&dir, &m.app, &["--write"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout)
            .contains("0 .mf2 file(s) written (1 unchanged); 0 Rust file(s) rewritten; 0 error(s)"),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(tree(&dir), before);

    // The dry run reports nothing to write or rewrite.
    let out = convert_app(&dir, &m.app, &[]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(!stdout.contains("would write"), "{stdout}");
    assert!(!stdout.contains("--- a/"), "{stdout}");
    assert!(
        stdout.contains("0 .mf2 file(s) to write (1 unchanged); 0 Rust file(s) to rewrite"),
        "{stdout}"
    );
    assert_eq!(tree(&dir), before);
}

#[test]
fn unchanged_text_is_not_a_rewrite_and_the_cookie_is_named() {
    // The file keeps naming `leptos_fluent` (its initializer is left for a
    // person), and its only call is in a view, where it stays as it is.
    let lib = "use leptos_fluent::{leptos_fluent, tr};\n\
               fn p() {\n    leptos_fluent! { locales: \"./locales\", cookie_name: \"lang\" }\n}\n\
               fn v() {\n    view! { <p>{tr!(\"hello\")}</p> }\n}\n";
    let dir = scratch("lf-unchanged");
    let app = dir.join("app");
    for (path, body) in [
        ("Cargo.toml", "[package]\nname = \"app\"\n"),
        ("locales/en/main.ftl", APP_FTL),
        ("src/lib.rs", lib),
    ] {
        let full = app.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }

    let first = convert_app(&dir, &app, &["--write"]);
    let stdout = text(&first.stdout);
    assert_eq!(first.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("0 Rust file(s) rewritten"), "{stdout}");
    assert_eq!(
        std::fs::read_to_string(app.join("src/lib.rs")).expect("read"),
        lib
    );
    // The initializer's cookie: read as an extra source, since the client
    // always writes `mf2_locale`.
    assert!(
        stdout.contains(
            "its `lang` cookie is not read: the client always writes `mf2_locale`, so to \
             keep the language readers chose before, add \
             `CookieLocale { name: \"lang\", ..Default::default() }` to the server's \
             `Negotiator` as an extra source"
        ),
        "{stdout}"
    );

    // A second `--write` exits as the first did, with the same report, and
    // writes nothing.
    let before = tree(&dir);
    let second = convert_app(&dir, &app, &["--write"]);
    assert_eq!(second.status.code(), first.status.code());
    let findings = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| l.contains(": error: "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(findings(&text(&second.stdout)), findings(&stdout));
    assert_eq!(tree(&dir), before);
}

/// `mf2 convert --from leptos-fluent` on `app`, into `dir/i18n`.
fn convert_app(dir: &Path, app: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "convert",
        "--from",
        "leptos-fluent",
        app.to_str().expect("utf-8"),
        "--i18n-crate",
        "app_i18n",
    ];
    args.extend_from_slice(extra);
    run(&dir.join("i18n"), &args)
}

/// Every file under `dir`, and its bytes.
fn tree(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).expect("read_dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = std::fs::read(&path).expect("read");
                out.insert(path, bytes);
            }
        }
    }
    out
}

// One test per rule (§6.2), named after it.

#[test]
fn rule_tr_string() {
    let m = migrate(
        "tr-string",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"hello\") }\nfn g() -> String { tr!(\"hello\").to_string() }\nfn h() -> String { format!(\"{}!\", tr!(\"hello\")) }\n",
    );
    m.clean();
    assert!(
        m.lib
            .contains("fn f() -> String { tr!(\"hello\").to_string() }"),
        "{}",
        m.lib
    );
    assert!(
        m.lib
            .contains("fn g() -> String { tr!(\"hello\").to_string() }"),
        "{}",
        m.lib
    );
    assert!(
        m.lib
            .contains("format!(\"{}!\", tr!(\"hello\").to_string())"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_tr_view() {
    let m = migrate(
        "tr-view",
        "use leptos_fluent::tr;\nfn v() { view! { <p>{tr!(\"hello\")}</p> <input placeholder=tr!(\"hello\") title={tr!(\"greet\", {\"name\" => who()})}/> <p>{tr!(\"hello\").len()}</p> } }\n",
    );
    m.clean();
    assert!(
        m.lib.contains("<p>{tr!(\"hello\")}</p> <input placeholder=tr!(\"hello\") title={tr!(\"greet\", name = who())}/> <p>{tr!(\"hello\").to_string().len()}</p>"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_move_tr_view() {
    let m = migrate(
        "move-tr-view",
        "use leptos_fluent::move_tr;\nfn v() { view! { <p>{move_tr!(\"greet\", {\"name\" => who})}</p> <Label text=move_tr!(\"hello\")/> } }\n",
    );
    m.clean();
    assert!(
        m.lib
            .contains("<p>{tr!(\"greet\", name = who)}</p> <Label text=tr!(\"hello\")/>"),
        "{}",
        m.lib
    );
    assert!(m.lib.starts_with("use app_i18n::tr;"), "{}", m.lib);
}

#[test]
fn rule_move_tr_signal() {
    let m = migrate(
        "move-tr-signal",
        "use leptos_fluent::move_tr;\nfn s() { let label = move_tr!(\"hello\"); view! { <p>{move_tr!(\"greet\", {\"name\" => name.get()})}</p> } }\n",
    );
    m.clean();
    assert!(
        m.lib
            .contains("let label = Signal::derive(move || tr!(\"hello\").to_string());"),
        "{}",
        m.lib
    );
    assert!(
        m.lib.contains(
            "<p>{Signal::derive(move || tr!(\"greet\", name = name.get()).to_string())}</p>"
        ),
        "{}",
        m.lib
    );
}

#[test]
fn rule_closure() {
    let m = migrate(
        "closure",
        "use leptos_fluent::tr;\nfn v() { view! { <p>{move || tr!(\"hello\")}</p> <p title=|| tr!(\"hello\")>{move || tr!(\"greet\", {\"name\" => name.get()})}</p> } }\n",
    );
    m.clean();
    assert!(
        m.lib.contains("<p>{tr!(\"hello\")}</p> <p title=tr!(\"hello\")>{move || tr!(\"greet\", name = name.get()).to_string()}</p>"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_arguments() {
    let m = migrate(
        "arguments",
        "use leptos_fluent::tr;\nfn f(x: i64) -> String { tr!(\"two\", { \"a-b\" => 1, \"c\" => x, }) + &tr!(\"hello\", {}) }\n",
    );
    m.clean();
    assert!(
        m.lib
            .contains("tr!(\"two\", \"a-b\" = 1, c = x).to_string() + &tr!(\"hello\").to_string()"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_context() {
    let m = migrate(
        "context",
        "use leptos_fluent::tr;\nfn f(i18n: u8) -> String { tr!(i18n, \"greet\", {\"name\" => \"Ada\"}) }\n",
    );
    m.clean();
    assert!(
        m.lib
            .contains("{ tr!(\"greet\", name = \"Ada\").to_string() }"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_attribute() {
    let m = migrate(
        "attribute",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"hint.before\") }\n",
    );
    m.clean();
    assert!(
        m.lib.contains("tr!(\"hint.before\").to_string()"),
        "{}",
        m.lib
    );
}

#[test]
fn rule_import() {
    let m = migrate(
        "import",
        "use leptos_fluent::{move_tr, tr};\nfn f() -> String { leptos_fluent::tr!(\"hello\") }\n",
    );
    m.clean();
    assert_eq!(
        m.lib,
        "use app_i18n::tr;\nfn f() -> String { app_i18n::tr!(\"hello\").to_string() }\n"
    );
}

// One test per code (§6.2, "Reported, not rewritten"), named after it.

#[test]
fn leptos_fluent_initializer() {
    // The `.ftl` directory is the initializer's `locales:`.
    let dir = scratch("lf-initializer");
    let app = dir.join("app");
    std::fs::create_dir_all(app.join("src")).expect("mkdir");
    std::fs::create_dir_all(app.join("i18n-files/en")).expect("mkdir");
    std::fs::write(app.join("Cargo.toml"), "[package]\nname = \"app\"\n").expect("write");
    std::fs::write(app.join("i18n-files/en/main.ftl"), APP_FTL).expect("write");
    std::fs::write(
        app.join("src/lib.rs"),
        "fn p() {\n    leptos_fluent! { locales: \"./i18n-files\", default_language: \"en\" }\n}\n",
    )
    .expect("write");
    let out = run(
        &dir.join("i18n"),
        &[
            "convert",
            "--from",
            "leptos-fluent",
            app.to_str().expect("utf-8"),
            "--i18n-crate",
            "x",
            "--write",
            "--format",
            "json",
        ],
    );
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert!(!out.status.success());
    let diagnostics = json["diagnostics"].as_array().expect("an array");
    assert_eq!(diagnostics.len(), 1, "{json:#}");
    assert_eq!(diagnostics[0]["code"], "leptos-fluent-initializer");
    at(&diagnostics[0], "error", 2, 5, None);
    assert!(dir.join("i18n/locales/en/main.mf2").is_file());
}

#[test]
fn leptos_fluent_context() {
    let m = migrate(
        "context-use",
        "fn s() {\n    let i18n = expect_context::<leptos_fluent::I18n>();\n}\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-context"), "error", 2, 48, None);
}

#[test]
fn leptos_fluent_import() {
    let m = migrate("import-other", "use leptos_fluent::{tr, I18n};\n");
    assert!(!m.ok);
    at(m.one("leptos-fluent-import"), "error", 1, 1, None);
    assert_eq!(m.lib, "use leptos_fluent::{tr, I18n};\n");
}

#[test]
fn leptos_fluent_dynamic_id() {
    let m = migrate(
        "dynamic",
        "use leptos_fluent::tr;\nfn f(id: &str) -> String { tr!(id) }\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-dynamic-id"), "error", 2, 32, None);
    assert!(m.lib.contains("{ tr!(id) }"));
}

#[test]
fn leptos_fluent_if_form() {
    let m = migrate(
        "if-form",
        "use leptos_fluent::tr;\nfn f(c: bool) -> String { tr!(if c { \"hello\" } else { \"greet\" }) }\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-if-form"), "error", 2, 31, None);
}

#[test]
fn leptos_fluent_cfg() {
    let m = migrate(
        "cfg",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"greet\", #[cfg(x)] {\"name\" => 1}) }\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-cfg"), "error", 2, 33, None);
}

#[test]
fn leptos_fluent_argument_name() {
    let m = migrate(
        "argument-name",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"greet\", {1 => 2}) }\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-argument-name"), "error", 2, 34, None);
}

#[test]
fn leptos_fluent_call() {
    let m = migrate(
        "call",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"a\", \"b\", \"c\", \"d\") }\n",
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-call"), "error", 2, 24, None);
}

#[test]
fn leptos_fluent_parse() {
    let m = migrate("parse", "use leptos_fluent::tr;\nfn f() { (\n");
    assert!(!m.ok);
    assert_eq!(m.one("leptos-fluent-parse")["level"], "error");
    assert_eq!(m.lib, "use leptos_fluent::tr;\nfn f() { (\n");
}

#[test]
fn leptos_fluent_dependency() {
    let m = migrate_with(
        "dependency",
        "[package]\nname = \"app\"\n\n[dependencies]\nleptos-fluent = \"0.3\"\n",
        "",
        &[],
    );
    assert!(!m.ok);
    at(m.one("leptos-fluent-dependency"), "error", 5, 1, None);
}

#[test]
fn leptos_fluent_unknown_id() {
    let m = migrate(
        "unknown-id",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"nope\") }\n",
    );
    assert!(!m.ok);
    at(
        m.one("leptos-fluent-unknown-id"),
        "error",
        2,
        24,
        Some("nope"),
    );
    // Still rewritten: the compiler will say the same.
    assert!(m.lib.contains("tr!(\"nope\").to_string()"));
}

#[test]
fn leptos_fluent_arguments() {
    let m = migrate(
        "arguments-check",
        "use leptos_fluent::tr;\nfn f() -> String { tr!(\"greet\", {\"who\" => 1}) }\n",
    );
    assert!(!m.ok);
    let d = m.one("leptos-fluent-arguments");
    at(d, "error", 2, 20, Some("greet"));
    let message = d["message"].as_str().expect("a message");
    assert!(
        message.contains("`who`") && message.contains("`name`"),
        "{message}"
    );
}
