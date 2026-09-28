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

/// Makes `dir` a cargo package, the i18n crate `mf2 compile --site` reads
/// its features from, with `default` as its default features.
fn i18n_crate(dir: &Path, default: &str) {
    let manifest = format!(
        "[package]\nname = \"cli-i18n\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [features]\ndefault = [{default}]\nfn-number = []\nfn-datetime = []\n\
         datetime-icu = [\"fn-datetime\"]\nintl = []\n\n\
         # Not a member of the repository's workspace.\n[workspace]\n"
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).expect("write");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir");
    std::fs::write(dir.join("src/lib.rs"), "").expect("write");
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
fn compile_site_writes_the_catalogs_and_their_index_and_nothing_else() {
    let dir = corpus("cli-compile-site");
    i18n_crate(&dir, "\"fn-number\"");
    let site = dir.join("site/i18n");
    let args = [
        "compile",
        "--features",
        "fn-number",
        "--site",
        site.to_str().expect("utf-8"),
    ];
    let text = ok(&run(&dir, &args));
    assert!(text.contains("1600 messages, 4 locales"), "{text}");
    // Nothing a static host should not serve: no manifest, no module, and
    // nothing in the default `--out`.
    assert!(!site.join("manifest.mf2m").exists());
    assert!(!site.join("mf2_generated.rs").exists());
    assert!(!dir.join("dist").exists());

    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(site.join("index.json")).expect("index.json"))
            .expect("json");
    let index = index.as_object().expect("an object");
    assert_eq!(index.len(), 4, "one entry per locale: {index:?}");
    for (tag, file) in index {
        let file = file.as_str().expect("a file name");
        assert!(file.starts_with(&format!("{tag}.")), "{tag}: {file}");
        for suffix in ["", ".br", ".gz"] {
            assert!(
                site.join(format!("{file}{suffix}")).is_file(),
                "{file}{suffix}"
            );
        }
    }

    // A second publish changes nothing; a changed translation replaces its
    // catalog, and the old one goes.
    assert!(ok(&run(&dir, &args)).contains("(0 file(s) changed)"));
    let path = dir.join("locales/pl/common.mf2");
    let source = std::fs::read_to_string(&path).expect("read");
    let (id, _) = source
        .lines()
        .find_map(|line| line.split_once(" = "))
        .expect("a simple message");
    let edited = source.replacen(&format!("{id} = "), &format!("{id} = edited "), 1);
    std::fs::write(&path, edited).expect("write");
    let verbose = ok(&run(&dir, &[&args[..], &["--verbose"]].concat()));
    assert!(verbose.contains("removed "), "{verbose}");
    let catalogs = std::fs::read_dir(&site)
        .expect("read_dir")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "mf2b"))
        .count();
    assert_eq!(catalogs, 4, "the stale catalog is gone");
}

#[test]
fn compile_site_builds_for_the_i18n_crates_features_and_rejects_others() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-compile-site-features");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("locales/en")).expect("mkdir");
    std::fs::write(dir.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
    // `:percent` is a build error without `fn-number`.
    std::fs::write(
        dir.join("locales/en/main.mf2"),
        "@locale en\n---\n\nshare = {$n :percent}\n",
    )
    .expect("write");
    let site = dir.join("site/i18n");
    let site = site.to_str().expect("utf-8");

    // Not a cargo package: nothing to take the functions from.
    let out = run(
        &dir,
        &["compile", "--features", "fn-number", "--site", site],
    );
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stderr(&out).contains("no Cargo.toml"), "{}", stderr(&out));

    // Without `--features`, cargo's: `fn-number` is a default, so `:percent`
    // builds.
    i18n_crate(&dir, "\"fn-number\"");
    ok(&run(&dir, &["compile", "--site", site]));
    // The same list, or one that differs only in what no catalog depends on,
    // is accepted.
    ok(&run(
        &dir,
        &["compile", "--features", "fn-number,intl", "--site", site],
    ));

    // One that differs is rejected, naming both lists, and writes nothing.
    std::fs::remove_dir_all(dir.join("site")).expect("rm");
    let out = run(
        &dir,
        &[
            "compile",
            "--features",
            "fn-number,fn-datetime",
            "--site",
            site,
        ],
    );
    assert!(!out.status.success(), "{}", stdout(&out));
    let err = stderr(&out);
    assert!(
        err.contains("--features names [fn-datetime, fn-number]")
            && err.contains("cargo resolves [fn-number] for cli-i18n"),
        "{err}"
    );
    assert!(!dir.join("site").exists());

    // Cargo's features are the crate's as the build enables them: with
    // `fn-number` no longer a default, `--features fn-number` disagrees…
    i18n_crate(&dir, "");
    let out = run(
        &dir,
        &["compile", "--features", "fn-number", "--site", site],
    );
    assert!(
        stderr(&out).contains("cargo resolves no function features"),
        "{}",
        stderr(&out)
    );
    // …and without `--features` the build fails on `:percent`, as cargo's
    // build of the crate would.
    let out = run(&dir, &["compile", "--site", site]);
    assert!(!out.status.success(), "{}", stdout(&out));
}

/// A corpus in a fresh directory: per locale, the body of its `main.mf2`.
fn small_corpus(name: &str, locales: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    for (tag, body) in locales {
        std::fs::create_dir_all(dir.join("locales").join(tag)).expect("mkdir");
        std::fs::write(
            dir.join("locales").join(tag).join("main.mf2"),
            format!("@locale {tag}\n---\n\n{body}"),
        )
        .expect("write");
    }
    std::fs::write(dir.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
    dir
}

#[test]
fn check_takes_the_i18n_crates_features_from_cargo() {
    // Without `fn-number`, `:percent` is an error (and `:integer` would be a
    // `neutral-numbers` warning, once the corpus has no errors).
    let dir = small_corpus(
        "cli-check-features",
        &[("en", "share = {$n :percent}\nitems = {$n :integer}\n")],
    );

    // Not a cargo package: checked with no features, and a note says so,
    // on stderr (the JSON stays one document).
    let out = run(&dir, &["check"]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("gated-function"), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("note: checking with no function features")
            && stderr(&out).contains("no Cargo.toml"),
        "{}",
        stderr(&out)
    );
    let out = run(&dir, &["check", "--format", "json"]);
    let _: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON document");

    // The i18n crate turns `fn-number` on by default: a bare check is
    // clean, as the build is, and says nothing about cargo.
    i18n_crate(&dir, "\"fn-number\"");
    let out = run(&dir, &["check"]);
    let text = ok(&out);
    assert!(text.contains("nothing to report"), "{text}");
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));

    // `--features` still wins over cargo's.
    let out = run(&dir, &["check", "--features", ""]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("gated-function"), "{}", stdout(&out));

    // …and cargo's are the crate's as the build enables them.
    i18n_crate(&dir, "");
    let out = run(&dir, &["check"]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("gated-function"), "{}", stdout(&out));
    ok(&run(&dir, &["check", "--features", "fn-number"]));
}

#[test]
fn check_names_the_first_missing_translations() {
    let source = (0..13)
        .map(|n| format!("m{n:02} = Text"))
        .collect::<Vec<_>>()
        .join("\n");
    let dir = small_corpus(
        "cli-check-missing",
        &[("en", &source), ("fr", "m00 = Texte\n")],
    );
    let text = ok(&run(&dir, &["check"]));
    assert!(
        text.contains(
            "12 of 13 messages are missing here and fall back to en: \
             m01, m02, m03, m04, m05, m06, m07, m08, m09, m10, and 2 more \
             (locale fr) [missing-translation]"
        ),
        "{text}"
    );
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
