//! Every command of `mf2`, on the reference workload (Phase 5a, A8).
//!
//! The binary is run as a user runs it — arguments in, exit status and
//! output out — so what is tested is the command line, not the library
//! behind it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use workload_gen::{Knobs, Workload, locale_files};

/// The `mf2` binary cargo built for this test.
fn mf2() -> PathBuf {
    // `CARGO_BIN_EXE_<name>` is set for every binary of the crate under test.
    PathBuf::from(env!("CARGO_BIN_EXE_mf2"))
}

/// A fresh copy of the reference workload, so that a test that writes cannot
/// disturb another.
fn corpus(name: &str) -> PathBuf {
    static SOURCE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let source = SOURCE.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("workload");
        if root.join("mf2.toml").is_file() {
            return root;
        }
        let wl = Workload::generate(&Knobs::default()).expect("the reference workload");
        let files = locale_files(&wl).expect("its files");
        for (path, bytes) in files.iter() {
            if !path.starts_with("locales/") {
                continue;
            }
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&full, bytes).expect("write");
        }
        std::fs::write(root.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
        root
    });
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&target);
    copy_dir(source, &target);
    target
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

/// Runs `mf2` with `args`.
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(mf2())
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("the mf2 binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn ok(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
    stdout(output)
}

#[test]
fn check_reports_what_it_finds_and_exits_zero_on_a_clean_corpus() {
    let dir = corpus("cli-check");
    let out = run(&dir, &["check", "--features", "fn-number"]);
    let text = ok(&out);
    assert!(text.contains("0 error(s)"), "{text}");
    // The pseudo-locales do not spell out Arabic's plural categories, which
    // is a warning and not an error.
    assert!(text.contains("missing-plural-category"), "{text}");

    let json = ok(&run(
        &dir,
        &["check", "--features", "fn-number", "--format", "json"],
    ));
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert!(
        value["diagnostics"].as_array().expect("an array").len() > 5,
        "{json}"
    );
}

#[test]
fn check_fails_on_a_corpus_with_an_error() {
    let dir = corpus("cli-check-bad");
    let path = dir.join("locales/pl/common.mf2");
    let text = std::fs::read_to_string(&path).expect("read");
    std::fs::write(&path, format!("{text}\nbroken = a {{$x\n")).expect("write");
    let out = run(&dir, &["check"]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("syntax error"), "{}", stdout(&out));
}

#[test]
fn compile_writes_a_catalog_per_locale_and_the_generated_module() {
    let dir = corpus("cli-compile");
    let out = dir.join("dist");
    let text = ok(&run(
        &dir,
        &[
            "compile",
            "--features",
            "fn-number",
            "-o",
            out.to_str().expect("utf-8"),
        ],
    ));
    assert!(text.contains("1600 messages, 4 locales"), "{text}");
    assert!(out.join("manifest.mf2m").is_file());
    assert!(out.join("mf2_generated.rs").is_file());
    let catalogs: Vec<_> = std::fs::read_dir(&out)
        .expect("read_dir")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "mf2b"))
        .collect();
    assert_eq!(catalogs.len(), 4, "one catalog per locale");
}

#[test]
fn stats_reports_coverage_sizes_and_the_pins() {
    let dir = corpus("cli-stats");
    let text = ok(&run(&dir, &["stats", "--features", "fn-number"]));
    assert!(text.contains("1600 messages"), "{text}");
    assert!(text.contains("CLDR 48.2.1"), "{text}");
    assert!(text.contains("MF2 spec 5c4ddb27"), "{text}");
    assert!(text.contains("plural.cardinal"), "{text}");
    // The reference workload's manifest hash is the figure plans/02 §3
    // records for it.
    assert!(text.contains("0x43e0dc12eeb05ef1"), "{text}");

    let json = ok(&run(&dir, &["stats", "--format", "json"]));
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(value["messages"], 1600);
    assert_eq!(value["locales"].as_array().expect("an array").len(), 4);
    assert!(value["locales"][0]["br"].as_u64().expect("a number") > 0);
}

#[test]
fn fmt_leaves_the_generated_corpus_alone() {
    // The layout `mf2 fmt` writes is the one `bench/workload-gen` writes, so
    // a generated corpus is already canonical — which is what makes `--check`
    // usable in CI.
    let dir = corpus("cli-fmt");
    let text = ok(&run(&dir, &["fmt", "--check"]));
    assert!(text.contains("0 of 72 file(s) would change"), "{text}");

    // Undo the layout of one file; `fmt` puts it back.
    let path = dir.join("locales/en/hotkeys.mf2");
    let before = std::fs::read_to_string(&path).expect("read");
    std::fs::write(&path, before.replace(" = ", "  =  ")).expect("write");
    let out = run(&dir, &["fmt", "--check"]);
    assert!(!out.status.success(), "a changed file should fail --check");
    ok(&run(&dir, &["fmt"]));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
}

#[test]
fn export_and_import_round_trip_a_locale() {
    let dir = corpus("cli-exchange");
    let json = dir.join("pl.json");
    ok(&run(
        &dir,
        &["export", "pl", "-o", json.to_str().expect("utf-8")],
    ));
    let exported = std::fs::read_to_string(&json).expect("read");
    assert!(exported.starts_with("{\n  \""), "{}", &exported[..40]);

    // Change one message in the export and read it back.
    let changed = exported.replacen("\": \"", "\": \"CHANGED ", 1);
    std::fs::write(&json, &changed).expect("write");
    let out = run(
        &dir,
        &["import", "pl", json.to_str().expect("utf-8"), "--dry-run"],
    );
    assert!(
        stderr(&out).contains("1 message(s) would change"),
        "{}",
        stderr(&out)
    );
    ok(&run(&dir, &["import", "pl", json.to_str().expect("utf-8")]));

    // And the locale now says so, with its comments and properties intact.
    let again = ok(&run(&dir, &["export", "pl"]));
    assert!(again.contains("CHANGED "), "the import did not land");
    let file = std::fs::read_to_string(dir.join("locales/pl/common.mf2")).expect("read");
    assert!(file.contains("@param"), "the properties were lost");
    assert!(file.contains('#'), "the comments were lost");
}

#[test]
fn dump_decodes_a_catalog_back_to_messages() {
    let dir = corpus("cli-dump");
    let out = dir.join("dist");
    ok(&run(&dir, &["compile", "-o", out.to_str().expect("utf-8")]));
    let catalog = std::fs::read_dir(&out)
        .expect("read_dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("en."))
                && p.extension().is_some_and(|e| e == "mf2b")
        })
        .expect("en's catalog");

    let manifest = out.join("manifest.mf2m");
    let text = ok(&run(
        &dir,
        &[
            "dump",
            catalog.to_str().expect("utf-8"),
            "--manifest",
            manifest.to_str().expect("utf-8"),
        ],
    ));
    assert_eq!(text.lines().count(), 1600);
    assert!(
        text.contains("app.canary.zq7-canary-msg = "),
        "{}",
        &text[..200]
    );

    let json = ok(&run(
        &dir,
        &[
            "dump",
            catalog.to_str().expect("utf-8"),
            "--manifest",
            manifest.to_str().expect("utf-8"),
            "--format",
            "json",
            "--id",
            "app.canary.zq7-canary-msg",
        ],
    ));
    let value: serde_json::Value = serde_json::from_str(json.trim()).expect("json");
    assert_eq!(value["id"], "app.canary.zq7-canary-msg");
    assert_eq!(value["message"]["type"], "message");
}

#[test]
fn pseudo_writes_the_two_pseudo_locales() {
    let dir = corpus("cli-pseudo");
    // Start from a corpus that has only the source locale, so that what the
    // command writes is what is there afterwards.
    for tag in ["pl", "en-XA", "ar-XB"] {
        std::fs::remove_dir_all(dir.join("locales").join(tag)).expect("rm");
    }
    let out = run(&dir, &["pseudo"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("en-XA"), "{}", stderr(&out));

    let en_xa = std::fs::read_to_string(dir.join("locales/en-XA/common.mf2")).expect("read");
    assert!(en_xa.contains("@locale en-XA"), "{}", &en_xa[..120]);
    assert!(en_xa.contains('['), "the accented locale brackets its text");
    let ar_xb = std::fs::read_to_string(dir.join("locales/ar-XB/common.mf2")).expect("read");
    assert!(
        ar_xb.contains('\u{202e}'),
        "the RTL locale overrides its runs"
    );

    // And what it wrote is a corpus that builds.
    ok(&run(&dir, &["check"]));
}

#[test]
fn init_scaffolds_a_crate_that_the_other_commands_understand() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-init");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let text = ok(&run(&dir, &["init", "--locale", "pl", "--locale", "de"]));
    assert!(text.contains("watch-additional-files"), "{text}");
    for file in ["mf2.toml", "Cargo.toml", "build.rs", "src/lib.rs"] {
        assert!(dir.join(file).is_file(), "{file}");
    }
    assert!(dir.join("locales/en/main.mf2").is_file());
    assert!(dir.join("locales/de/main.mf2").is_file());

    // A second `init` refuses to overwrite.
    let out = run(&dir, &["init"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--force"), "{}", stderr(&out));

    // What it scaffolded checks and compiles.
    let checked = ok(&run(&dir, &["check"]));
    assert!(checked.contains("0 error(s)"), "{checked}");
    ok(&run(&dir, &["compile", "-o", "dist"]));
}

#[test]
fn watch_rebuilds_when_a_locale_changes() {
    let dir = corpus("cli-watch");
    let path = dir.join("locales/pl/common.mf2");
    let dir_for_thread = dir.clone();
    let editor = std::thread::spawn(move || {
        // Give the watch its first build, then change a message.
        std::thread::sleep(std::time::Duration::from_millis(400));
        let text = std::fs::read_to_string(&path).expect("read");
        std::fs::write(&path, text.replace("Unknown and new", "Nowy")).expect("write");
        drop(dir_for_thread);
    });
    let out = run(
        &dir,
        &[
            "watch",
            "-o",
            "dist",
            "--interval",
            "60",
            "--max-rebuilds",
            "1",
        ],
    );
    editor.join().expect("the editing thread");
    let text = ok(&out);
    assert!(text.contains("mf2 watch:"), "{text}");
    assert!(
        text.contains("common.mf2"),
        "the change was not noticed:\n{text}"
    );
    assert_eq!(
        text.matches("messages, 4 locales").count(),
        2,
        "expected a first build and a rebuild:\n{text}"
    );
}
