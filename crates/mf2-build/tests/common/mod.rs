//! The reference workload on disk, written once per test binary.

// Each test binary includes this module and uses part of it.
#![allow(dead_code, unreachable_pub)]

use std::path::{Path, PathBuf};

use workload_gen::{Knobs, Workload, locale_files};

/// Every locale the default knobs produce, in tag order.
pub const TAGS: [&str; 4] = ["ar-XB", "en", "en-XA", "pl"];

/// Writes the reference workload under cargo's target tmp directory and
/// gives its root. The tests run in parallel in one process, so this happens
/// behind a `OnceLock`.
pub fn workload() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("corpus");
        let marker = root.join(".written");
        if marker.is_file() {
            return root;
        }
        let wl = Workload::generate(&Knobs::default()).expect("the reference workload");
        let files = locale_files(&wl).expect("its files");
        for (path, bytes) in files.iter() {
            // The i18n crate's layout: locales/<tag>/*.mf2 beside mf2.toml.
            if !path.starts_with("locales/") {
                continue;
            }
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&full, bytes).expect("write");
        }
        std::fs::write(
            root.join("mf2.toml"),
            "source_locale = \"en\"\n\n[catalog]\nstrip = [\"cold\", \"ids\"]\nmissing = \"fallback\"\n",
        )
        .expect("write mf2.toml");
        std::fs::write(&marker, "written by tests/common/mod.rs").expect("write");
        root
    })
    .clone()
}

/// A fresh output directory under the target tmp directory.
pub fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("out")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// A corpus of hand-written locales: `locales/<tag>/main.mf2` per entry.
pub fn corpus(name: &str, config: &str, locales: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("corpora")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("mkdir");
    std::fs::write(root.join("mf2.toml"), config).expect("write");
    for (tag, text) in locales {
        let dir = root.join("locales").join(tag);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("main.mf2"), text).expect("write");
    }
    root
}
