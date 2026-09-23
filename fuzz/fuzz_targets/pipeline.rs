//! Fuzz target `pipeline`: the whole build pipeline on generated corpora.
//!
//! The input is one locale's resource file. The target makes a two-locale
//! corpus from it — the source, and a translation holding every other entry,
//! so that fallback flattening has something to do — writes it, and runs
//! `mf2-build` over it. Then, for every corpus the build **accepted**:
//!
//! * every catalog loads under the manifest's hash and decodes, message by
//!   message, to the model the flattened corpus says it should be (the
//!   translation's where it has one, the source's where it falls back);
//! * the fallback flag says which of the two it was;
//! * a second build gives byte-identical catalogs (F8), and writes nothing;
//! * the generated module names no catalog outside its `ssr` block (B6).
//!
//! A corpus the build **refused** only has to have said why: a report with
//! at least one error, and no catalog written.
//!
//! Non-UTF-8 input is fed through `from_utf8_lossy`, so every run exercises
//! the pipeline.

#![no_main]

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;
use mf2_build::loader::Loader;
use mf2_build::{Build, Config, Features, Level, Lint};

/// Where the corpus is written. One directory, rewritten every run: the
/// files are small and the kernel keeps them in cache.
fn corpus_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("mf2-fuzz-build");
    let _ = std::fs::create_dir_all(dir.join("locales").join("en"));
    let _ = std::fs::create_dir_all(dir.join("locales").join("pl"));
    dir
}

/// Every other entry of `text`, as a translation: enough of the source to
/// give the fallback chain both cases.
fn half(text: &str) -> String {
    let mut out = String::from("@locale pl\n---\n");
    for (i, line) in text.lines().enumerate() {
        // Only whole single-line entries: a continuation would be cut.
        if i % 2 == 0 && line.contains(" = ") && !line.starts_with(char::is_whitespace) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let dir = corpus_dir();
    let en = dir.join("locales/en/main.mf2");
    let pl = dir.join("locales/pl/main.mf2");
    if std::fs::write(&en, text.as_bytes()).is_err() || std::fs::write(&pl, half(&text)).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("mf2.toml"), "source_locale = \"en\"\n");

    let mut config = Config::default();
    // The lints are not what this target is about: a corpus that trips one
    // would never reach the writer, and the writer is what it fuzzes.
    for lint in Lint::ALL {
        if lint.floor() == Level::Allow {
            config.lints.insert(*lint, Level::Allow);
        }
    }
    let out = std::env::temp_dir().join("mf2-fuzz-build-out");
    let features = Features::parse("fn-number");
    let build = || {
        Build::at(&dir, &out)
            .config(config.clone())
            .features(features.clone())
            .run()
    };

    let Ok(outcome) = build() else {
        // An I/O or layout failure is the harness's, not the corpus's.
        return;
    };
    if !outcome.is_clean() {
        assert!(
            outcome.report.errors() > 0,
            "a refused corpus reported no error"
        );
        assert!(
            outcome.catalogs.is_empty(),
            "a refused corpus wrote {} catalog(s)",
            outcome.catalogs.len()
        );
        return;
    }

    // What the corpus says each locale's messages are.
    let sources = |tag: &str| -> Vec<(String, String)> {
        mf2_build::loader::resource::Resources
            .load(&dir.join("locales").join(tag))
            .map(|loaded| {
                loaded
                    .records
                    .iter()
                    .map(|r| (r.id.clone(), r.source.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let en_sources = sources("en");
    let pl_sources = sources("pl");

    for catalog in &outcome.catalogs {
        let loaded = mf2_catalog::Catalog::new(catalog.bytes.clone(), outcome.manifest_hash)
            .expect("a catalog the build wrote loads under its own manifest");
        assert_eq!(loaded.message_count() as usize, outcome.manifest.ids.len());
        assert_eq!(loaded.locale(), catalog.tag);

        let own = if catalog.tag == "pl" {
            &pl_sources
        } else {
            &en_sources
        };
        for (index, id) in outcome.manifest.ids.iter().enumerate() {
            let msg = mf2_catalog::MsgId::from_raw(index as u32);
            let decoded = mf2_catalog::decode(&loaded, msg).expect("every message decodes");
            // The flattened model: this locale's, or the source's.
            let (source, fell_back) = match own.iter().find(|(i, _)| i == id) {
                Some((_, source)) => (source.clone(), false),
                None => (
                    en_sources
                        .iter()
                        .find(|(i, _)| i == id)
                        .map(|(_, s)| s.clone())
                        .expect("the source locale has every id"),
                    catalog.tag != "en",
                ),
            };
            let expected = mf2_syntax::parse_model(&source)
                .message
                .expect("a message the build accepted parses");
            assert_eq!(
                mf2_syntax::serialize(&decoded).ok(),
                mf2_syntax::serialize(&expected).ok(),
                "{}/{id} decoded to something else",
                catalog.tag
            );
            assert_eq!(
                loaded.fallback_locale(msg).is_some(),
                fell_back,
                "{}/{id}: the fallback flag is wrong",
                catalog.tag
            );
        }
    }

    // The generated module keeps the catalogs behind `ssr` (B6).
    check_generated(&outcome.generated);

    // Deterministic, and a second build writes nothing (F8, P0.9).
    let again = build().expect("a second build");
    assert_eq!(again.manifest_hash, outcome.manifest_hash);
    assert!(
        again.written.is_empty(),
        "a second build rewrote {:?}",
        again.written
    );
    for (a, b) in outcome.catalogs.iter().zip(&again.catalogs) {
        assert_eq!(a.bytes, b.bytes, "{} is not deterministic", a.tag);
        assert_eq!(a.br, b.br, "{}'s brotli is not deterministic", a.tag);
    }
    assert_eq!(again.generated, outcome.generated);
});

/// No catalog name outside the module's `ssr` block.
fn check_generated(generated: &str) {
    let mut gated = false;
    for line in generated.lines() {
        if line.starts_with("#[cfg(feature = \"ssr\")]") {
            gated = true;
        }
        if line.contains(".mf2b") {
            assert!(gated, "a catalog is named outside an ssr block: {line}");
        }
        // The block ends at the next item that is not part of it.
        if gated && line == "}" {
            gated = false;
        }
    }
}
