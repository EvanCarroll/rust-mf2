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
fn i18n_crate(dir: &Path, features: &str) {
    // A stand-in `mf2` with the features that decide the functions: the
    // command reads the set cargo resolves for it, as the build does
    // through `links`.
    std::fs::create_dir_all(dir.join("mf2/src")).expect("mkdir");
    std::fs::write(
        dir.join("mf2/Cargo.toml"),
        "[package]\nname = \"mf2\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [features]\nfn-number = []\nfn-datetime = []\n\
         datetime-icu = [\"fn-datetime\"]\nintl = []\n",
    )
    .expect("write");
    std::fs::write(dir.join("mf2/src/lib.rs"), "").expect("write");
    let manifest = format!(
        "[package]\nname = \"cli-i18n\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nmf2 = {{ path = \"mf2\", features = [{features}] }}\n\n\
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

    // Without `--features`, cargo's: the crate turns `mf2`'s `fn-number`
    // on, so `:percent` builds.
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
            && err.contains("cargo resolves [fn-number] for mf2 in cli-i18n"),
        "{err}"
    );
    assert!(!dir.join("site").exists());

    // Cargo's features are mf2's as the build enables them: with `fn-number`
    // no longer on, `--features fn-number` disagrees…
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

    // Not a cargo package: checked as if every function were on, so no
    // function is reported as gated that a build may have, and a note says
    // so, on stderr (the JSON stays one document).
    let out = run(&dir, &["check"]);
    ok(&out);
    assert!(!stdout(&out).contains("gated-function"), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("checks as if every function were on")
            && stderr(&out).contains("no Cargo.toml"),
        "{}",
        stderr(&out)
    );
    let out = run(&dir, &["check", "--format", "json"]);
    let _: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON document");

    // The crate turns `mf2`'s `fn-number` on: a bare check is clean, as the
    // build is, and says nothing about cargo.
    i18n_crate(&dir, "\"fn-number\"");
    let out = run(&dir, &["check"]);
    let text = ok(&out);
    assert!(text.contains("nothing to report"), "{text}");
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));

    // `--features` still wins over cargo's.
    let out = run(&dir, &["check", "--features", ""]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("gated-function"), "{}", stdout(&out));

    // …and cargo's are mf2's as the build enables them.
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

/// The UX review's case (Phase 10 E1): the French terms sentence lost its
/// link. `check` refuses it and names the markup; with the link kept, the
/// same corpus passes.
#[test]
fn check_refuses_a_translation_that_drops_markup() {
    let dir = small_corpus(
        "cli-check-dropped-markup",
        &[
            ("en", "terms = Accept our {#link}terms{/link}.\n"),
            ("fr", "terms = Acceptez nos conditions.\n"),
        ],
    );
    let out = run(&dir, &["check"]);
    assert!(!out.status.success(), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.contains("the source message has {#link}, which this translation leaves out")
            && text.contains("(in terms, locale fr) [dropped-markup]"),
        "{text}"
    );

    std::fs::write(
        dir.join("locales/fr/main.mf2"),
        "@locale fr\n---\n\nterms = Acceptez nos {#link}conditions{/link}.\n",
    )
    .expect("write");
    ok(&run(&dir, &["check"]));
}

/// The UX review's case (Phase 10 E2): French has one of four messages, and
/// one of the three it lacks is a language's own name, marked
/// `@do-not-translate`. `check` and `stats`, as text and as JSON, count two
/// of three missing.
#[test]
fn do_not_translate_messages_are_not_missing() {
    let dir = small_corpus(
        "cli-do-not-translate",
        &[
            (
                "en",
                "greeting = Hello\nfarewell = Goodbye\napply = Apply\n\n\
                 @do-not-translate\nlanguage-fr = Français\n",
            ),
            ("fr", "greeting = Bonjour\n"),
        ],
    );
    let text = ok(&run(&dir, &["check"]));
    assert!(
        text.contains("2 of 3 messages are missing here and fall back to en: apply, farewell"),
        "{text}"
    );

    let text = ok(&run(&dir, &["stats"]));
    assert!(
        text.contains("4 messages (1 marked @do-not-translate)"),
        "{text}"
    );
    let row = |tag: &str| -> Vec<String> {
        text.lines()
            .find(|l| l.split_whitespace().next() == Some(tag))
            .unwrap_or_else(|| panic!("no row for {tag}: {text}"))
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(row("fr")[1..3], ["33.3%", "2"], "{text}");
    assert_eq!(row("en")[1..3], ["100.0%", "0"], "{text}");

    let json = ok(&run(&dir, &["stats", "--format", "json"]));
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(value["messages"], 4);
    assert_eq!(value["do_not_translate"], 1);
    let fr = value["locales"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|l| l["locale"] == "fr")
        .expect("fr");
    assert_eq!((&fr["messages"], &fr["missing"]), (&1.into(), &2.into()));
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
fn stats_takes_the_i18n_crates_features_from_cargo() {
    // `:integer` formats with CLDR's number symbols only with `fn-number`,
    // so whether the catalog carries them says which features stats used.
    let dir = small_corpus("cli-stats-features", &[("en", "items = {$n :integer}\n")]);
    let symbols = |dir: &Path| -> bool {
        let out = run(dir, &["stats", "--format", "json"]);
        let value: serde_json::Value = serde_json::from_str(&ok(&out)).expect("json");
        value["locales"][0]["locale_data"]
            .as_array()
            .expect("an array")
            .iter()
            .any(|e| e["entry"] == "number.symbols")
    };
    // The crate turns `mf2`'s `fn-number` on, or leaves it off: stats
    // counts what that build ships, as `check` checks it.
    i18n_crate(&dir, "\"fn-number\"");
    assert!(symbols(&dir));
    i18n_crate(&dir, "");
    assert!(!symbols(&dir));
    // `--features` still wins over cargo's.
    let out = run(
        &dir,
        &["stats", "--features", "fn-number", "--format", "json"],
    );
    assert!(ok(&out).contains("number.symbols"));
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

/// The UX review's case (Phase 10 E3): a French translation with a variable
/// its source does not declare (`$nom`). `import` checks the files as they
/// would be, as `check` does, and writes nothing. A clean translation lands
/// (the negative control), and an error the file already had does not stop
/// an import that brings none.
#[test]
fn import_refuses_what_check_would_refuse() {
    let dir = small_corpus(
        "cli-import-checked",
        &[
            (
                "en",
                "greeting = Hello, {$name}!\nfarewell = Goodbye, {$name}!\n",
            ),
            (
                "fr",
                "greeting = Bonjour, {$name} !\nfarewell = Au revoir, {$name} !\n",
            ),
        ],
    );
    let fr = dir.join("locales/fr/main.mf2");
    let json = dir.join("fr.json");
    let import = |body: &str| {
        std::fs::write(&json, body).expect("write");
        run(&dir, &["import", "fr", json.to_str().expect("utf-8")])
    };
    let read = || std::fs::read_to_string(&fr).expect("read");
    let before = read();

    let out = import("{\"greeting\": \"Bonjour, {$nom} !\"}\n");
    assert!(!out.status.success(), "{}", stderr(&out));
    let text = stderr(&out);
    assert!(
        text.contains("$nom is not an input of the source message")
            && text.contains("(in greeting, locale fr) [undeclared-variable]")
            && text.contains("nothing was written"),
        "{text}"
    );
    assert_eq!(read(), before);

    let out = import("{\"greeting\": \"Salut, {$name} !\"}\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(read().contains("greeting = Salut, {$name} !"), "{}", read());

    std::fs::write(
        &fr,
        read().replace("Au revoir, {$name}", "Au revoir, {$nom}"),
    )
    .expect("write");
    let out = import("{\"greeting\": \"Coucou, {$name} !\"}\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        read().contains("greeting = Coucou, {$name} !"),
        "{}",
        read()
    );
}

/// JSON import changes the messages a language has. One it does not have
/// yet needs a file and a section, which flat JSON does not say: it is left
/// out, named, with XLIFF as the way to add it, and the command exits 1
/// after writing the rest (Phase 10 E3). An id the source lacks is named
/// apart.
#[test]
fn import_names_xliff_for_messages_a_language_lacks() {
    let dir = small_corpus(
        "cli-import-new",
        &[
            ("en", "greeting = Hello\nfarewell = Goodbye\n"),
            ("fr", "greeting = Bonjour\n"),
        ],
    );
    let json = dir.join("fr.json");
    std::fs::write(
        &json,
        "{\"farewell\": \"Au revoir\", \"greeting\": \"Salut\", \"mystery\": \"?\"}\n",
    )
    .expect("write");
    let out = run(&dir, &["import", "fr", json.to_str().expect("utf-8")]);
    assert!(!out.status.success(), "{}", stderr(&out));
    let text = stderr(&out);
    assert!(
        text.contains("1 message(s) fr does not have yet were left out: farewell")
            && text.contains("`mf2 export fr --format xliff`"),
        "{text}"
    );
    assert!(
        text.contains("1 id(s) are not messages of en and were left out: mystery"),
        "{text}"
    );
    let fr = std::fs::read_to_string(dir.join("locales/fr/main.mf2")).expect("read");
    assert!(fr.contains("greeting = Salut"), "{fr}");
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

/// A message marked `@do-not-translate` (itself or its section) is copied
/// into the pseudo-locales as it stands, so that they pass `check`.
#[test]
fn pseudo_copies_do_not_translate_messages() {
    let dir = small_corpus(
        "cli-pseudo-dnt",
        &[(
            "en",
            "greeting = Hello\n\n@do-not-translate\nlanguage-fr = Français\n\n\
             @do-not-translate\n[brand]\nname = Example\n",
        )],
    );
    ok(&run(&dir, &["pseudo"]));
    let en_xa = std::fs::read_to_string(dir.join("locales/en-XA/main.mf2")).expect("read");
    assert!(en_xa.contains("language-fr = Français\n"), "{en_xa}");
    assert!(en_xa.contains("name = Example\n"), "{en_xa}");
    assert!(!en_xa.contains("greeting = Hello"), "{en_xa}");
    ok(&run(&dir, &["check"]));
}

#[test]
fn init_without_a_mode_names_the_modes_and_changes_nothing() {
    let dir = fresh("cli-init-none");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let out = run(&dir, &["init", "--locale", "fr"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    for flag in ["--cli", "--tui", "--ssr", "--islands", "--csr"] {
        assert!(err.contains(flag), "{err}");
    }
    assert_eq!(std::fs::read_dir(&dir).expect("read_dir").count(), 0);
    // One mode at a time.
    let out = run(&dir, &["init", "--ssr", "--csr"]);
    assert!(!out.status.success());
}

#[test]
fn init_web_modes_make_a_new_application() {
    let root = fresh("cli-init-web");
    std::fs::create_dir_all(&root).expect("mkdir");
    let text = ok(&run(&root, &["init", "--ssr", "my-app"]));
    assert!(text.contains("cargo leptos watch"), "{text}");
    let app = root.join("my-app");
    let manifest = std::fs::read_to_string(app.join("Cargo.toml")).expect("manifest");
    for line in [
        "features = [\"leptos\", \"fn-number\"]",
        "\"mf2/ssr\",\n    \"mf2/axum\",",
        "watch-additional-files = [\"locales\"]",
        "output-name = \"my_app\"",
        "[profile.dev.build-override]\nopt-level = 2",
    ] {
        assert!(manifest.contains(line), "{line}: {manifest}");
    }
    let server = std::fs::read_to_string(app.join("src/main.rs")).expect("main");
    assert!(server.contains("my_app::install();"), "{server}");
    assert!(app.join("src/lib.rs").is_file());

    ok(&run(&root, &["init", "--islands", "isles"]));
    let manifest = std::fs::read_to_string(root.join("isles/Cargo.toml")).expect("manifest");
    assert!(manifest.contains("\"static-locale\""), "{manifest}");
    assert!(manifest.contains("features = [\"islands\"]"), "{manifest}");

    let text = ok(&run(&root, &["init", "--csr", "client"]));
    assert!(text.contains("trunk serve"), "{text}");
    let index = std::fs::read_to_string(root.join("client/index.html")).expect("index");
    assert!(index.contains("data-bin=\"client\""), "{index}");
    assert!(root.join("client/Trunk.toml").is_file());
    for app in ["my-app", "isles", "client"] {
        let locales = root.join(app).join("locales");
        ok(&run(
            &root,
            &["fmt", "--check", &locales.display().to_string()],
        ));
    }
}

/// A fresh, missing directory under the test's temporary directory.
fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn init_cli_and_tui_make_a_new_application() {
    let root = fresh("cli-init-new");
    std::fs::create_dir_all(&root).expect("mkdir");
    let text = ok(&run(&root, &["init", "--cli", "count"]));
    assert!(text.contains("cargo run -- --lang fr"), "{text}");
    let manifest = std::fs::read_to_string(root.join("count/Cargo.toml")).expect("manifest");
    assert!(manifest.contains("name = \"count\""), "{manifest}");
    assert!(
        manifest.contains("features = [\"native\", \"fn-number\"]"),
        "{manifest}"
    );
    assert!(
        manifest.contains("[profile.dev.build-override]\nopt-level = 2"),
        "{manifest}"
    );
    for file in [
        "build.rs",
        "src/main.rs",
        "locales/en/main.mf2",
        "locales/fr/main.mf2",
    ] {
        assert!(root.join("count").join(file).is_file(), "{file}");
    }
    // The directory is no longer empty, and holds a crate: another `init`
    // would add translations to it, and refuses to overwrite them.
    let out = run(&root.join("count"), &["init", "--cli"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--force"), "{}", stderr(&out));

    ok(&run(&root, &["init", "--tui", "hops"]));
    let manifest = std::fs::read_to_string(root.join("hops/Cargo.toml")).expect("manifest");
    assert!(manifest.contains("ratatui = \"0.30\""), "{manifest}");
    assert!(root.join("hops/src/ui.rs").is_file());
    // Its messages are in `mf2 fmt`'s form.
    for app in ["count", "hops"] {
        let locales = root.join(app).join("locales");
        ok(&run(
            &root,
            &["fmt", "--check", &locales.display().to_string()],
        ));
    }

    // A new application's languages are its messages'.
    let out = run(&root, &["init", "--cli", "de", "--locale", "de"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--locale"), "{}", stderr(&out));
    // A directory with files and no crate is not one to write into.
    std::fs::write(root.join("notes.txt"), "x").expect("write");
    let out = run(&root, &["init", "--cli"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("neither empty nor a crate"),
        "{}",
        stderr(&out)
    );
}

#[cfg(unix)]
#[test]
fn init_tui_adds_translations_to_an_existing_crate() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = fresh("cli-init-existing");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").expect("main");
    // A cargo that writes down what it was asked to do.
    let log = dir.join("cargo.log");
    let cargo = dir.join("fake-cargo");
    std::fs::write(
        &cargo,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
    )
    .expect("fake cargo");
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let out = Command::new(mf2())
        .arg("-C")
        .arg(&dir)
        .args(["init", "--tui", "--locale", "fr"])
        .env("CARGO", &cargo)
        .output()
        .expect("the mf2 binary runs");
    let text = ok(&out);
    assert!(
        text.contains("[profile.dev.build-override]\nopt-level = 2"),
        "{text}"
    );
    assert!(text.contains("use crate::prelude::*;"), "{text}");
    let asked = std::fs::read_to_string(&log).expect("cargo ran");
    assert_eq!(
        asked, "add mf2@2 -F native,ratatui\nadd --build mf2-build@2\n",
        "{asked}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("build.rs")).expect("build.rs"),
        "fn main() {\n    mf2_build::run();\n}\n"
    );
    assert!(dir.join("locales/en/main.mf2").is_file());
    assert!(dir.join("locales/fr/main.mf2").is_file());
    assert!(
        !dir.join("mf2.toml").exists(),
        "en is the default source locale"
    );
    // The application's own files are left alone.
    assert_eq!(
        std::fs::read_to_string(dir.join("src/main.rs")).expect("main"),
        "fn main() {}\n"
    );
}

#[cfg(unix)]
#[test]
fn init_ssr_adds_translations_to_an_existing_crate() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = fresh("cli-init-existing-web");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"site\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    let log = dir.join("cargo.log");
    let cargo = dir.join("fake-cargo");
    std::fs::write(
        &cargo,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
    )
    .expect("fake cargo");
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let out = Command::new(mf2())
        .arg("-C")
        .arg(&dir)
        .args(["init", "--ssr", "--no-messages", "--locale", "fr"])
        .env("CARGO", &cargo)
        .output()
        .expect("the mf2 binary runs");
    let text = ok(&out);
    for line in [
        "\"mf2/ssr\", \"mf2/axum\"",
        "\"mf2/hydrate\"",
        "watch-additional-files = [\"locales\"]",
        "Negotiator::default()",
    ] {
        assert!(text.contains(line), "{line}: {text}");
    }
    let asked = std::fs::read_to_string(&log).expect("cargo ran");
    assert_eq!(
        asked, "add mf2@2 -F leptos,fn-number\nadd --build mf2-build@2\n",
        "{asked}"
    );
    assert!(dir.join("build.rs").is_file());
    // Room for a conversion: no starter messages.
    assert!(!dir.join("locales").exists());
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
