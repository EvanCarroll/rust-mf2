//! `mf2 export --format xliff` and `mf2 import` of XLIFF 2 (Phase 8 A6;
//! the tooling design §6.3), run as a user runs them.
//!
//! Every document exported here is validated with `xmllint` against the
//! vendored core schema; `xmllint` missing fails the test, it does not skip
//! it. One test per import code, named after it (checked below).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use workload_gen::{Knobs, Workload, locale_files};

const SCHEMA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../third_party/xliff/schemas/xliff_core_2.0.xsd"
);

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

fn ok(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        text(&output.stdout),
        text(&output.stderr)
    );
    text(&output.stderr)
}

/// `xmllint --noout --schema` on `file`.
fn validate(file: &Path) -> Output {
    Command::new("xmllint")
        .args(["--noout", "--schema", SCHEMA])
        .arg(file)
        .output()
        .expect(
            "xmllint must be installed (libxml2): every XLIFF export is validated against \
             the vendored schema, the tooling design §6.3",
        )
}

fn assert_valid(file: &Path) {
    let out = validate(file);
    assert!(
        out.status.success(),
        "{} does not validate:\n{}",
        file.display(),
        text(&out.stderr)
    );
}

/// A fresh directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("xliff")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

const EN: &str = "\
@locale en
---

hello = Hello, {$name}!

# The count of files in the folder.
@param $count - How many files.
files =
  .input {$count :integer}
  .match $count
  one {{{$count} file}}
  *   {{{$count} files}}

@do-not-translate
brand = Chattyness

[inbox]

new =
  .input {$count :integer}
  .match $count
  one {{{$count} new message}}
  *   {{{$count} new messages}}

bold = Press {#b}here{/b} now
";

const PL: &str = "\
@locale pl
---

hello = Cześć, {$name}!

files =
  .input {$count :integer}
  .match $count
  one {{{$count} plik}}
  *   {{{$count} plików}}
";

/// A small corpus: `en` and a `pl` that has two of its five messages.
fn corpus(name: &str) -> PathBuf {
    let dir = scratch(name);
    for (path, body) in [
        ("mf2.toml", "source_locale = \"en\"\n"),
        ("locales/en/main.mf2", EN),
        ("locales/pl/main.mf2", PL),
    ] {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }
    dir
}

/// Exports `pl`, validated; returns the document's path and text.
fn export(dir: &Path) -> (PathBuf, String) {
    let file = dir.join("pl.xlf");
    ok(&run(
        dir,
        &[
            "export",
            "--format",
            "xliff",
            "pl",
            "-o",
            file.to_str().expect("utf-8"),
        ],
    ));
    assert_valid(&file);
    let doc = std::fs::read_to_string(&file).expect("read");
    (file, doc)
}

/// The document with a `<target>` given to unit `unit`.
fn fill(doc: &str, unit: &str, target: &str) -> String {
    let at = doc
        .find(&format!("<unit id=\"{unit}\""))
        .unwrap_or_else(|| panic!("no unit {unit}"));
    let end = at + doc[at..].find("</source>").expect("a source") + "</source>".len();
    format!("{}<target>{target}</target>{}", &doc[..end], &doc[end..])
}

/// Imports `doc` into `pl`.
fn import(dir: &Path, doc: &str) -> Output {
    let file = dir.join("in.xlf");
    std::fs::write(&file, doc).expect("write");
    run(dir, &["import", "pl", file.to_str().expect("utf-8")])
}

fn pl(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("locales/pl/main.mf2")).expect("read")
}

/// The export's shape: a unit per message, a group per section and per
/// message that selects, Polish plural forms, codes for every expression,
/// notes for context, `translate="no"` where the source says so.
#[test]
fn the_export_has_the_mapping_of_the_plan() {
    let dir = corpus("shape");
    let (_, doc) = export(&dir);
    for expected in [
        r#"<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.1" srcLang="en" trgLang="pl" xml:space="preserve">"#,
        r#"<file id="f1" original="main.mf2" canResegment="no">"#,
        r#"<unit id="hello" name="hello">"#,
        r#"<source>Hello, <ph id="1" dataRef="d1" disp="{$name}"/>!</source>"#,
        r#"<target>Cześć, <ph id="1" dataRef="d1" disp="{$name}"/>!</target>"#,
        r#"<group id="files" name="files" type="mf2:select">"#,
        r#"<note category="comment">The count of files in the folder.</note>"#,
        r#"<note category="param">$count - How many files.</note>"#,
        r#"<unit id="files:1" name="one">"#,
        r#"<unit id="files:2" name="*">"#,
        r#"<unit id="files:3" name="few">"#,
        r#"<unit id="files:4" name="many">"#,
        r#"<unit id="brand" name="brand" translate="no">"#,
        r#"<group id="s:inbox" name="inbox" type="mf2:section">"#,
        r#"<source><ph id="1" dataRef="d1" disp="{$count}"/> new messages</source>"#,
        r#"<source>Press <pc id="1" dataRefStart="d1" dataRefEnd="d2" dispStart="{#b}" dispEnd="{/b}" type="fmt">here</pc> now</source>"#,
    ] {
        assert!(doc.contains(expected), "missing {expected}\n{doc}");
    }
    // Polish `few` and `many` show English `*`, and have no target yet.
    let few = &doc[doc.find(r#"<unit id="files:3""#).expect("few")..];
    let few = &few[..few.find("</unit>").expect("end")];
    assert!(few.contains("<source><ph id=\"1\" dataRef=\"d1\" disp=\"{$count}\"/> files</source>"));
    assert!(!few.contains("<target>"));
    // A message marked @do-not-translate is `translate="no"` and has no
    // target: it is no work for a tool to count, as it is not missing for
    // `check` and `stats` (Phase 10 E2).
    let brand = &doc[doc.find(r#"<unit id="brand""#).expect("brand")..];
    let brand = &brand[..brand.find("</unit>").expect("end")];
    assert!(!brand.contains("<target>"), "{brand}");
}

/// Owner question 7 in practice: a translator fills Polish `few` of an
/// existing message and translates two new ones; each lands where it
/// belongs — the variant before the catch-all, the new messages in the
/// section and order the source has them.
#[test]
fn a_translated_target_lands_in_its_message_and_variant() {
    let dir = corpus("lands");
    let (_, doc) = export(&dir);
    let ph = r#"<ph id="1" dataRef="d1"/>"#;
    let mut doc = fill(&doc, "files:3", &format!("{ph} pliki"));
    for (unit, text) in [
        ("inbox.new:1", "nowa wiadomość"),
        ("inbox.new:2", "nowe wiadomości"),
        ("inbox.new:4", "nowej wiadomości"),
    ] {
        doc = fill(&doc, unit, &format!("{ph} {text}"));
    }
    // `many` left empty: absent, and the reader gets `*` for it.
    doc = fill(
        &doc,
        "inbox.bold",
        r#"Naciśnij <pc id="1" dataRefStart="d1" dataRefEnd="d2">tutaj</pc> teraz"#,
    );
    let stderr = ok(&import(&dir, &doc));
    assert!(stderr.contains("1 message(s) changed, 2 added"), "{stderr}");
    let after = pl(&dir);
    // A changed message is written by the serializer, in `mf2 fmt`'s form.
    assert!(
        after
            .contains("  one {{{$count} plik}}\n  few {{{$count} pliki}}\n  * {{{$count} plików}}"),
        "{after}"
    );
    let inbox = &after[after.find("[inbox]").expect("the section is added")..];
    assert!(
        inbox.contains("new =\n  .input {$count :integer}\n  .match $count\n  one {{{$count} nowa wiadomość}}\n  few {{{$count} nowe wiadomości}}\n  * {{{$count} nowej wiadomości}}"),
        "{inbox}"
    );
    assert!(
        inbox.find("new =") < inbox.find("bold = Naciśnij {#b}tutaj{/b} teraz"),
        "{inbox}"
    );
    assert!(!after.contains("brand"), "do-not-translate is not copied");
    ok(&run(&dir, &["check"]));
    ok(&run(&dir, &["fmt", "--check"]));

    // Exported again and imported back, nothing changes.
    let before = pl(&dir);
    let (_, again) = export(&dir);
    // The target's own variants first (one, few, *), then the form it lacks.
    assert!(
        again.contains(r#"<unit id="inbox.new:4" name="many">"#),
        "{again}"
    );
    let stderr = ok(&import(&dir, &again));
    assert!(stderr.contains("0 message(s) changed, 0 added"), "{stderr}");
    assert_eq!(pl(&dir), before);
}

/// The reference workload: every target locale exported, validated and
/// imported back leaves every resource byte-identical.
#[test]
fn the_reference_workload_round_trips_byte_identical() {
    let dir = scratch("workload");
    let wl = Workload::generate(&Knobs::default()).expect("the reference workload");
    let files = locale_files(&wl).expect("its files");
    let mut originals = Vec::new();
    for (path, bytes) in files.iter() {
        if !path.starts_with("locales/") {
            continue;
        }
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, bytes).expect("write");
        originals.push((full, bytes.to_vec()));
    }
    std::fs::write(dir.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
    for tag in ["pl", "en-XA", "ar-XB"] {
        let file = dir.join(format!("{tag}.xlf"));
        ok(&run(
            &dir,
            &[
                "export",
                "--format",
                "xliff",
                tag,
                "-o",
                file.to_str().expect("utf-8"),
            ],
        ));
        assert_valid(&file);
        let stderr = ok(&run(&dir, &["import", tag, file.to_str().expect("utf-8")]));
        assert!(
            stderr.contains("0 message(s) changed, 0 added"),
            "{tag}: {stderr}"
        );
    }
    for (path, bytes) in originals {
        assert!(
            std::fs::read(&path).expect("read") == bytes,
            "{} changed",
            path.display()
        );
    }
}

/// Negative control for the validation: a document the schema does not
/// allow — a unit without its required id, an element XLIFF does not have —
/// is rejected, so a passing `assert_valid` means something.
#[test]
fn validation_rejects_what_the_schema_does_not_allow() {
    let dir = corpus("invalid");
    let (_, doc) = export(&dir);
    for broken in [
        doc.replacen(
            r#"<unit id="hello" name="hello">"#,
            r#"<unit name="hello">"#,
            1,
        ),
        doc.replacen("<segment", "<segmnt", 1)
            .replacen("</segment>", "</segmnt>", 1),
        doc.replacen(r#"canResegment="no""#, r#"canResegment="maybe""#, 1),
    ] {
        assert_ne!(broken, doc);
        let file = dir.join("broken.xlf");
        std::fs::write(&file, &broken).expect("write");
        assert!(!validate(&file).status.success(), "{broken}");
    }
}

/// An edited protected code is refused, with the unit named; the message
/// stays as it was, and the rest of the document still lands.
#[test]
fn xliff_code_edited() {
    let dir = corpus("code-edited");
    let (_, doc) = export(&dir);
    let unit = doc.find(r#"<unit id="hello""#).expect("hello");
    let data = unit
        + doc[unit..]
            .find(r#"<data id="d1">{$name}</data>"#)
            .expect("data");
    let doc = format!(
        "{}{}",
        &doc[..data],
        doc[data..].replacen("{$name}", "{$name :string}", 1)
    );
    let doc = doc.replacen("Cześć, <ph", "Hej, <ph", 1);
    let doc = fill(&doc, "files:3", r#"<ph id="1" dataRef="d1"/> pliki"#);
    let out = import(&dir, &doc);
    assert!(!out.status.success());
    let stderr = text(&out.stderr);
    assert!(stderr.contains("xliff-code-edited: f1/hello:"), "{stderr}");
    let after = pl(&dir);
    assert!(after.contains("hello = Cześć, {$name}!"), "{after}");
    assert!(after.contains("few {{{$count} pliki}}"), "{after}");
}

/// A code with neither data nor a base to copy — a code a tool invented —
/// has no MF2 meaning.
#[test]
fn xliff_unknown_code() {
    let dir = corpus("unknown-code");
    let (_, doc) = export(&dir);
    let doc = doc.replacen(
        r#"<target>Cześć, <ph id="1" dataRef="d1" disp="{$name}"/>!</target>"#,
        r#"<target>Cześć, <ph id="9"/>!</target>"#,
        1,
    );
    let out = import(&dir, &doc);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("xliff-unknown-code: f1/hello:"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(pl(&dir), PL);
    // A copy of an existing code is fine: it means the same expression.
    let (_, doc) = export(&dir);
    let doc = doc.replacen(
        r#"<target>Cześć, <ph id="1" dataRef="d1" disp="{$name}"/>!</target>"#,
        r#"<target>Cześć, <ph id="1" dataRef="d1"/> i <ph id="2" copyOf="1"/>!</target>"#,
        1,
    );
    ok(&import(&dir, &doc));
    assert!(pl(&dir).contains("hello = Cześć, {$name} i {$name}!"));
}

/// A unit or a variant the export does not have: a stale document.
#[test]
fn xliff_unknown_unit() {
    let dir = corpus("unknown-unit");
    let (_, doc) = export(&dir);
    let doc = fill(&doc, "files:3", r#"<ph id="1" dataRef="d1"/> pliki"#)
        .replacen(
            r#"<unit id="files:3" name="few">"#,
            r#"<unit id="files:3" name="two">"#,
            1,
        )
        .replacen(r#"<unit id="hello""#, r#"<unit id="goodbye""#, 1);
    let out = import(&dir, &doc);
    assert!(!out.status.success());
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("xliff-unknown-unit: f1/goodbye:"),
        "{stderr}"
    );
    assert!(
        stderr.contains("xliff-unknown-unit: f1/files:3:"),
        "{stderr}"
    );
    assert_eq!(pl(&dir), PL);
}

/// A `translate="no"` unit whose target differs from its source.
#[test]
fn xliff_do_not_translate() {
    let dir = corpus("do-not-translate");
    let (_, doc) = export(&dir);
    let out = import(&dir, &fill(&doc, "brand", "Gadatliwość"));
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("xliff-do-not-translate: f1/brand:"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(pl(&dir), PL);
    // Its source copied as the target is not a change.
    ok(&import(&dir, &fill(&doc, "brand", "Chattyness")));
    assert_eq!(pl(&dir), PL);
}

/// The UX review's first case (Phase 10 E1, E3): a tool dropped the link's
/// code from the French terms sentence. The document is well-formed and
/// every code it keeps is the export's, but the message it makes leaves out
/// `{#link}`: import checks the file as it would be, refuses, and writes
/// nothing. With the link kept, it lands (the negative control).
#[test]
fn import_refuses_a_translation_that_drops_markup() {
    let dir = scratch("dropped-markup");
    for (path, body) in [
        ("mf2.toml", "source_locale = \"en\"\n"),
        (
            "locales/en/main.mf2",
            "@locale en\n---\n\nterms = Accept our {#link}terms{/link}.\n",
        ),
        ("locales/fr/main.mf2", "@locale fr\n---\n"),
    ] {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }
    let file = dir.join("fr.xlf");
    ok(&run(
        &dir,
        &[
            "export",
            "--format",
            "xliff",
            "fr",
            "-o",
            file.to_str().expect("utf-8"),
        ],
    ));
    assert_valid(&file);
    let doc = std::fs::read_to_string(&file).expect("read");
    let fr = || std::fs::read_to_string(dir.join("locales/fr/main.mf2")).expect("read");
    let import = |doc: &str| {
        std::fs::write(&file, doc).expect("write");
        run(&dir, &["import", "fr", file.to_str().expect("utf-8")])
    };

    let out = import(&fill(&doc, "terms", "Acceptez nos conditions."));
    assert!(!out.status.success());
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("the source message has {#link}, which this translation leaves out")
            && stderr.contains("[dropped-markup]")
            && stderr.contains("nothing was written"),
        "{stderr}"
    );
    assert_eq!(fr(), "@locale fr\n---\n");

    let stderr = ok(&import(&fill(
        &doc,
        "terms",
        r#"Acceptez nos <pc id="1" dataRefStart="d1" dataRefEnd="d2">conditions</pc>."#,
    )));
    assert!(stderr.contains("0 message(s) changed, 1 added"), "{stderr}");
    assert!(
        fr().contains("terms = Acceptez nos {#link}conditions{/link}."),
        "{}",
        fr()
    );
}

/// A new message that selects, translated without its catch-all.
#[test]
fn xliff_incomplete() {
    let dir = corpus("incomplete");
    let (_, doc) = export(&dir);
    let doc = fill(
        &doc,
        "inbox.new:1",
        r#"<ph id="1" dataRef="d1"/> nowa wiadomość"#,
    );
    let out = import(&dir, &doc);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("xliff-incomplete: f1/inbox.new:1:"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(pl(&dir), PL);
}

/// Not XML, not XLIFF 2, or no target language: nothing is written.
#[test]
fn xliff_malformed() {
    let dir = corpus("malformed");
    let (_, doc) = export(&dir);
    for broken in [
        doc.replacen("</file>", "", 1),
        doc.replacen(r#"version="2.1""#, r#"version="1.2""#, 1),
        doc.replacen(r#" trgLang="pl""#, "", 1),
    ] {
        let out = import(
            &dir,
            &fill(&broken, "files:3", r#"<ph id="1" dataRef="d1"/> pliki"#),
        );
        assert!(!out.status.success());
        assert!(
            text(&out.stderr).contains("xliff-malformed:"),
            "{}",
            text(&out.stderr)
        );
        assert_eq!(pl(&dir), PL);
    }
}

/// A document for another language is not imported.
#[test]
fn a_document_for_another_locale_is_refused() {
    let dir = corpus("other-locale");
    let (_, doc) = export(&dir);
    let out = import(&dir, &doc.replacen(r#"trgLang="pl""#, r#"trgLang="de""#, 1));
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("not pl; nothing was written"),
        "{}",
        text(&out.stderr)
    );
    // And the source locale has nothing to translate.
    let out = run(&dir, &["export", "--format", "xliff", "en"]);
    assert!(!out.status.success());
}

/// A flat JSON locale is one `<file>`; a translation is written back into
/// it, and a new message is added to it.
#[test]
fn a_flat_json_locale_exchanges_too() {
    let dir = scratch("json");
    for (path, body) in [
        ("mf2.toml", "source_locale = \"en\"\n"),
        (
            "locales/en.json",
            "{\n  \"a\": \"Hello, {$name}!\",\n  \"b\": \"Bye\"\n}\n",
        ),
        ("locales/pl.json", "{\n  \"a\": \"Cześć, {$name}!\"\n}\n"),
    ] {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, body).expect("write");
    }
    let (_, doc) = export(&dir);
    assert!(
        doc.contains(r#"<file id="f1" original="en.json" canResegment="no">"#),
        "{doc}"
    );
    let stderr = ok(&import(&dir, &doc));
    assert!(stderr.contains("0 message(s) changed, 0 added"), "{stderr}");
    let stderr = ok(&import(&dir, &fill(&doc, "b", "Pa")));
    assert!(stderr.contains("0 message(s) changed, 1 added"), "{stderr}");
    let after = std::fs::read_to_string(dir.join("locales/pl.json")).expect("read");
    assert_eq!(
        after,
        "{\n  \"a\": \"Cześć, {$name}!\",\n  \"b\": \"Pa\"\n}\n"
    );
}

/// Every code `mf2 import` can report. An integration test cannot see the
/// crate's own `Finding`, so the set is literal here;
/// `src/workspace_tests.rs` holds this list against `Finding::ALL`, both its
/// length and every code in it, so it cannot drift from the code.
const IMPORT_CODES: [&str; 6] = [
    "xliff-code-edited",
    "xliff-unknown-code",
    "xliff-unknown-unit",
    "xliff-do-not-translate",
    "xliff-incomplete",
    "xliff-malformed",
];

/// Every import code has a test named after it here.
#[test]
fn every_code_has_a_test() {
    let me = include_str!("xliff.rs");
    for code in IMPORT_CODES {
        let test = format!("fn {}()", code.replace('-', "_"));
        assert!(me.contains(&test), "no test {test}");
    }
}
