//! The two loaders on the reference workload (Phase 5a, A2).
//!
//! `bench/workload-gen` writes the same 1,600 messages per locale twice —
//! as `.mf2` resources and as flat JSON — so the two loaders must give the
//! same records for them. It is the one corpus where a disagreement between
//! the container and what every translation-management system speaks would
//! show up.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mf2_build::loader::{Loader, json, resource};
use workload_gen::{Knobs, Workload, locale_files};

/// Writes the workload once per test binary, under cargo's target tmp
/// directory, and gives its root. The tests run in parallel in one process,
/// so the write happens behind a `OnceLock`: without it a test can read a
/// tree another is still writing.
fn workload() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("workload");
        let marker = root.join(".written");
        if marker.is_file() {
            return root;
        }
        let knobs = Knobs::default();
        let wl = Workload::generate(&knobs).expect("the reference workload");
        let files = locale_files(&wl).expect("its files");
        for (path, bytes) in files.iter() {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&full, bytes).expect("write");
        }
        std::fs::write(&marker, "written by tests/loaders.rs").expect("write");
        root
    })
    .clone()
}

/// `id → source` as a loader reads them, with every id unique.
fn pairs(loaded: &mf2_build::Loaded) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for record in &loaded.records {
        let previous = out.insert(record.id.clone(), record.source.clone());
        assert_eq!(previous, None, "{} appears twice", record.id);
    }
    out
}

const TAGS: [&str; 4] = ["en", "pl", "en-XA", "ar-XB"];

#[test]
fn the_two_loaders_agree_on_the_reference_workload() {
    let root = workload();
    for tag in TAGS {
        let from_files = resource::Resources
            .load(&root.join("locales").join(tag))
            .expect("the .mf2 files");
        let from_json = json::FlatJson
            .load(&root.join("json").join(format!("{tag}.json")))
            .expect("the flat JSON");
        assert!(
            from_files.problems.is_empty(),
            "{tag}: {:?}",
            from_files.problems
        );
        assert_eq!(from_files.records.len(), 1600, "{tag}: messages");
        assert_eq!(pairs(&from_files), pairs(&from_json), "{tag}");
    }
}

#[test]
fn every_resource_file_declares_its_locale() {
    let root = workload();
    for tag in TAGS {
        let loaded = resource::Resources
            .load(&root.join("locales").join(tag))
            .expect("the .mf2 files");
        assert_eq!(loaded.files.len(), 18, "{tag}: files");
        for (file, declared) in loaded.files.iter().zip(&loaded.declared) {
            assert_eq!(
                declared.as_deref(),
                Some(tag),
                "{}: @locale",
                file.path.display()
            );
        }
    }
}

#[test]
fn the_container_carries_translator_context() {
    let root = workload();
    let loaded = resource::Resources
        .load(&root.join("locales").join("en"))
        .expect("the .mf2 files");
    let with_comment = loaded
        .records
        .iter()
        .filter(|r| r.comment.is_some())
        .count();
    let with_param = loaded
        .records
        .iter()
        .filter(|r| r.meta.iter().any(|p| p.name == "param"))
        .count();
    assert!(with_comment > 800, "only {with_comment} comments");
    assert!(with_param > 100, "only {with_param} @param properties");
    eprintln!("en: {with_comment} comments, {with_param} @param properties");

    // Every `@param` names a variable the message actually uses.
    for record in &loaded.records {
        for property in record.meta.iter().filter(|p| p.name == "param") {
            let value = property.value.as_deref().expect("@param has a value");
            let name = value
                .strip_prefix('$')
                .expect("@param names a variable")
                .split(|c: char| !mf2_resource::is_id_char(c))
                .next()
                .expect("a name");
            assert!(
                record.source.contains(&format!("${name}")),
                "{}: @param ${name} is not in the message",
                record.id
            );
            assert!(record.param(name).is_some(), "{}: ${name}", record.id);
        }
    }

    // The canary's id, name and text are in the corpus, so the B6 grep of the
    // client wasm has something to look for.
    assert!(
        loaded
            .records
            .iter()
            .any(|r| r.id == "app.canary.zq7-canary-msg"),
        "the B6 canary"
    );
}

#[test]
fn a_locale_round_trips_through_its_json_export() {
    let root = workload();
    let loaded = resource::Resources
        .load(&root.join("locales").join("pl"))
        .expect("the .mf2 files");
    let exported = json::write(
        loaded
            .records
            .iter()
            .map(|r| (r.id.as_str(), r.source.as_str())),
    );
    let reread = json::read(&exported).expect("what we wrote reads");
    let there: BTreeMap<&str, &str> = reread
        .iter()
        .map(|p| (p.id.as_str(), p.source.as_str()))
        .collect();
    let here: BTreeMap<&str, &str> = loaded
        .records
        .iter()
        .map(|r| (r.id.as_str(), r.source.as_str()))
        .collect();
    assert_eq!(there, here);

    // And it is the file `workload-gen` writes, byte for byte.
    let written = std::fs::read_to_string(root.join("json").join("pl.json")).expect("read");
    assert_eq!(exported, written, "the export is not the generator's JSON");
}

#[test]
fn a_record_maps_an_offset_in_its_message_back_to_the_file() {
    let root = workload();
    for (loaded, what) in [
        (
            resource::Resources
                .load(&root.join("locales").join("en"))
                .expect("files"),
            "resource",
        ),
        (
            json::FlatJson
                .load(&root.join("json").join("en.json"))
                .expect("json"),
            "json",
        ),
    ] {
        for record in &loaded.records {
            let file = loaded.file(record);
            for (i, _) in record.source.char_indices() {
                let at = record.source_span(
                    u32::try_from(i).expect("a short message"),
                    u32::try_from(i).expect("a short message"),
                );
                assert!(
                    (at.start as usize) < file.text.len()
                        && file.text.is_char_boundary(at.start as usize),
                    "{what}: {} offset {i} maps outside {}",
                    record.id,
                    file.path.display()
                );
            }
        }
    }
}

#[test]
fn fmt_leaves_a_generated_file_alone_except_for_its_layout() {
    let root = workload();
    let path = root.join("locales").join("en").join("chat.mf2");
    let text = std::fs::read_to_string(&path).expect("read");
    let formatted = resource::format(&text).expect("a clean file formats");
    // Formatting is idempotent, and what it writes holds the same messages.
    assert_eq!(
        resource::format(&formatted).expect("formats again"),
        formatted
    );
    let before = mf2_resource::parse(&text).0;
    let after = mf2_resource::parse(&formatted).0;
    assert_eq!(before, after);
}
