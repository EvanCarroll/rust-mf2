//! The pipeline on the reference workload (Phase 5a, A4): the manifest, one
//! catalog per locale, fallbacks flattened, and every catalog decoding back
//! to the messages that went in.

mod common;

use std::collections::BTreeMap;

use common::{TAGS, corpus, out_dir, workload};
use mf2_build::{Build, Config, Features, Loader, Missing};
use mf2_catalog::Catalog;

fn features() -> Features {
    Features::parse("fn-number")
}

#[test]
fn the_reference_workload_builds() {
    let root = workload();
    let out = out_dir("workload");
    let outcome = Build::at(&root, &out)
        .features(features())
        .run()
        .expect("the workload builds");

    assert!(outcome.report.is_clean(), "{}", outcome.report.to_text());
    assert_eq!(outcome.manifest.ids.len(), 1600);
    assert_eq!(outcome.catalogs.len(), 4);
    assert_eq!(outcome.source_locale, "en");
    let tags: Vec<&str> = outcome.locales.iter().map(|l| l.tag.as_str()).collect();
    assert_eq!(tags, TAGS);

    // Ids are in `MsgId` order: bytewise ascending, dense.
    let mut sorted = outcome.manifest.ids.clone();
    sorted.sort();
    assert_eq!(sorted, outcome.manifest.ids);

    // Every locale has every message, so nothing falls back.
    for catalog in &outcome.catalogs {
        assert_eq!(catalog.missing, 0, "{}", catalog.tag);
        assert_eq!(catalog.fallbacks, 0, "{}", catalog.tag);
        assert!(!catalog.bytes.is_empty());
        assert!(catalog.br.len() < catalog.bytes.len(), "{}", catalog.tag);
        assert_eq!(catalog.hash.len(), 16);
        assert!(catalog.file_name().starts_with(&catalog.tag));
    }

    // The manifest and three files per locale landed in OUT_DIR.
    assert!(out.join("manifest.mf2m").is_file());
    for locale in &outcome.locales {
        let base = out.join(&locale.file_name);
        assert!(base.is_file(), "{}", base.display());
        assert!(base.with_extension("mf2b.br").is_file());
        assert!(base.with_extension("mf2b.gz").is_file());
    }
}

#[test]
fn every_catalog_decodes_to_the_messages_that_went_in() {
    let root = workload();
    let out = out_dir("decode");
    let outcome = Build::at(&root, &out)
        .features(features())
        .run()
        .expect("builds");

    // The source of truth: what the loader read for each locale.
    for catalog in &outcome.catalogs {
        let loaded = mf2_build::loader::resource::Resources
            .load(&root.join("locales").join(&catalog.tag))
            .expect("the locale's files");
        let sources: BTreeMap<&str, &str> = loaded
            .records
            .iter()
            .map(|r| (r.id.as_str(), r.source.as_str()))
            .collect();

        let read =
            Catalog::new(catalog.bytes.clone(), outcome.manifest_hash).expect("the catalog loads");
        assert_eq!(
            read.message_count() as usize,
            outcome.manifest.ids.len(),
            "{}",
            catalog.tag
        );
        for (index, id) in outcome.manifest.ids.iter().enumerate() {
            let msg = mf2_catalog::MsgId::from_raw(u32::try_from(index).expect("an index"));
            let decoded = mf2_catalog::decode(&read, msg).expect("it decodes");
            let expected = mf2_syntax::parse_model(sources[id.as_str()])
                .message
                .expect("the source parses");
            assert_eq!(
                mf2_syntax::serialize(&decoded).expect("serializes"),
                mf2_syntax::serialize(&expected).expect("serializes"),
                "{}/{id}",
                catalog.tag
            );
        }
    }
}

#[test]
fn a_second_build_writes_nothing_and_a_fresh_one_is_identical() {
    let root = workload();
    let out = out_dir("again");
    let first = Build::at(&root, &out)
        .features(features())
        .run()
        .expect("builds");
    assert!(!first.written.is_empty());

    let second = Build::at(&root, &out)
        .features(features())
        .run()
        .expect("builds again");
    assert!(
        second.written.is_empty(),
        "rewrote {:?} though nothing changed",
        second.written
    );
    assert!(second.removed.is_empty());

    // A build in another directory gives the same bytes (F8).
    let elsewhere = out_dir("elsewhere");
    let third = Build::at(&root, &elsewhere)
        .features(features())
        .run()
        .expect("builds elsewhere");
    assert_eq!(third.manifest_hash, first.manifest_hash);
    for (a, b) in first.catalogs.iter().zip(&third.catalogs) {
        assert_eq!(a.tag, b.tag);
        assert_eq!(a.hash, b.hash, "{}", a.tag);
        assert_eq!(a.bytes, b.bytes, "{}", a.tag);
        assert_eq!(a.br, b.br, "{}", a.tag);
        assert_eq!(a.gz, b.gz, "{}", a.tag);
    }
}

const SOURCE: &str = "\
@locale en
---
greeting = Hello
farewell = Bye
count =
  .input {$n :integer}
  .match $n
  one {{one thing}}
  *   {{{$n} things}}
";

#[test]
fn a_locale_that_lacks_a_message_falls_back() {
    let root = corpus(
        "fallback",
        "source_locale = \"en\"\n",
        &[
            ("en", SOURCE),
            ("pl", "@locale pl\n---\ngreeting = Czesc\n"),
        ],
    );
    let out = out_dir("fallback");
    let outcome = Build::at(&root, &out)
        .features(Features::default())
        .run()
        .expect("builds");

    let pl = outcome.catalog("pl").expect("pl");
    assert_eq!(pl.missing, 2);
    assert_eq!(pl.fallbacks, 2);
    // The report says so once, with the count.
    let text = outcome.report.to_text();
    assert!(
        text.contains("2 of 3 messages are missing here and fall back to en"),
        "{text}"
    );

    // And the fallback text is English, flagged as such.
    let catalog = Catalog::new(pl.bytes.clone(), outcome.manifest_hash).expect("loads");
    let index = outcome
        .manifest
        .ids
        .iter()
        .position(|id| id == "farewell")
        .expect("the id");
    let msg = mf2_catalog::MsgId::from_raw(u32::try_from(index).expect("an index"));
    assert_eq!(catalog.fallback_locale(msg), Some("en"));
}

#[test]
fn the_missing_policy_decides_what_a_gap_looks_like() {
    for (policy, expected) in [(Missing::Id, "farewell"), (Missing::Empty, "")] {
        let root = corpus(
            "missing",
            "source_locale = \"en\"\n",
            &[
                ("en", SOURCE),
                ("pl", "@locale pl\n---\ngreeting = Czesc\n"),
            ],
        );
        let out = out_dir("missing");
        let mut config = Config::default();
        config.catalog.missing = policy;
        let outcome = Build::at(&root, &out)
            .config(config)
            .features(Features::default())
            .run()
            .expect("builds");
        let pl = outcome.catalog("pl").expect("pl");
        let catalog = Catalog::new(pl.bytes.clone(), outcome.manifest_hash).expect("loads");
        let index = outcome
            .manifest
            .ids
            .iter()
            .position(|id| id == "farewell")
            .expect("the id");
        let msg = mf2_catalog::MsgId::from_raw(u32::try_from(index).expect("an index"));
        let decoded = mf2_catalog::decode(&catalog, msg).expect("decodes");
        assert_eq!(
            mf2_syntax::serialize(&decoded).expect("serializes"),
            expected,
            "{policy:?}"
        );
        assert_eq!(pl.fallbacks, 0, "{policy:?}");
        assert_eq!(pl.missing, 2, "{policy:?}");
    }
}

#[test]
fn an_id_the_source_lacks_fails_the_build() {
    let root = corpus(
        "extra-id",
        "source_locale = \"en\"\n",
        &[
            ("en", SOURCE),
            ("pl", "@locale pl\n---\ngreeting = Czesc\nmystery = ?\n"),
        ],
    );
    let out = out_dir("extra-id");
    let outcome = Build::at(&root, &out)
        .features(Features::default())
        .check()
        .expect("the corpus is readable");
    assert!(!outcome.is_clean());
    let text = outcome.report.to_text();
    assert!(
        text.contains("the source locale \"en\" has no message with this id"),
        "{text}"
    );
    assert!(text.contains("[extra-id]"), "{text}");
    // And nothing was written, even though this was a `run`.
    let error = outcome.into_result().expect_err("a build script fails");
    assert!(error.to_string().contains("1 error"), "{error}");
}

#[test]
fn the_manifest_hash_ignores_a_translation_edit() {
    let root = corpus(
        "hash",
        "source_locale = \"en\"\n",
        &[
            ("en", SOURCE),
            ("pl", "@locale pl\n---\ngreeting = Czesc\n"),
        ],
    );
    let out = out_dir("hash");
    let before = Build::at(&root, &out)
        .features(Features::default())
        .run()
        .expect("builds");

    std::fs::write(
        root.join("locales").join("pl").join("main.mf2"),
        "@locale pl\n---\ngreeting = Dzien dobry\n",
    )
    .expect("edit");
    let after = Build::at(&root, &out)
        .features(Features::default())
        .run()
        .expect("builds again");

    assert_eq!(
        before.manifest_hash, after.manifest_hash,
        "a translation edit changed the manifest hash"
    );
    assert_eq!(
        before.catalog("en").expect("en").hash,
        after.catalog("en").expect("en").hash,
        "a translation edit changed another locale's catalog"
    );
    assert_ne!(
        before.catalog("pl").expect("pl").hash,
        after.catalog("pl").expect("pl").hash,
        "the edited locale's catalog did not change"
    );
    // The old catalog is gone, so nothing serves it by accident.
    assert!(
        after.removed.iter().any(|p| p
            .to_string_lossy()
            .contains(&before.catalog("pl").expect("pl").hash)),
        "the old pl catalog was left behind"
    );
}
