//! A small corpus for the native store's tests, written as `mf2-build`
//! writes one — one manifest, one catalog per language — and embedded as a
//! generated `CORPUS` embeds it, each catalog also given under its
//! content-hashed file name for the tests that read catalogs from a
//! directory. Included by `#[path]` from each test that needs it.

#![allow(dead_code, reason = "each test uses the part it needs")]

use mf2::{
    ArgValue, CatalogFile, Corpus, Dir, Function, MsgId, Registry, Tr, TrArgs, functions, tr,
    tr_args1,
};
use mf2_catalog::Manifest;
use mf2_catalog::writer::{self, Options};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("integer", &functions::INTEGER)];
/// The core `:integer`, with neutral digits: the catalogs carry no locale
/// data.
pub(crate) static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// The messages, in `MsgId` order: `welcome` (simple), `hello` (a string
/// argument, `name`), `files` (an integer, `n`).
pub(crate) const MESSAGES: [(&str, &[&str]); 3] = [("m0", &[]), ("m1", &["name"]), ("m2", &["n"])];

/// Each language: its tag, its direction, and its three messages.
pub(crate) const LANGUAGES: [(&str, Dir, [&str; 3]); 2] = [
    (
        "en",
        Dir::Ltr,
        ["Welcome", "Hello, {$name}!", "{$n :integer} files"],
    ),
    (
        "fr",
        Dir::Ltr,
        ["Bienvenue", "Bonjour, {$name} !", "{$n :integer} fichiers"],
    ),
];

/// `welcome`.
pub(crate) fn welcome() -> Tr {
    tr(MsgId::from_raw(0))
}

/// `hello`, with `name`.
pub(crate) fn hello(name: &'static str) -> TrArgs {
    tr_args1(MsgId::from_raw(1), ArgValue::str_static(name))
}

/// `files`, with `n`.
pub(crate) fn files(n: i64) -> TrArgs {
    tr_args1(MsgId::from_raw(2), ArgValue::from(n))
}

/// The corpus of [`LANGUAGES`] (the first is the source), with its catalogs
/// embedded or not, and each catalog's file name and bytes.
pub(crate) fn corpus(embed: bool) -> (&'static Corpus, Vec<(&'static str, Vec<u8>)>) {
    corpus_of(&LANGUAGES, embed)
}

/// The corpus of `languages`, as [`corpus`].
pub(crate) fn corpus_of(
    languages: &[(&'static str, Dir, [&str; 3])],
    embed: bool,
) -> (&'static Corpus, Vec<(&'static str, Vec<u8>)>) {
    let manifest = Manifest {
        ids: MESSAGES.iter().map(|(id, _)| (*id).to_owned()).collect(),
        slots: MESSAGES
            .iter()
            .map(|(_, slots)| slots.iter().map(|s| (*s).to_owned()).collect())
            .collect(),
        markup: vec![Vec::new(); MESSAGES.len()],
        functions: vec!["integer".to_owned()],
    };
    let mut locales = Vec::new();
    let mut files = Vec::new();
    let mut raw = Vec::new();
    for &(tag, dir, sources) in languages {
        let parsed: Vec<_> = sources
            .iter()
            .map(|source| {
                mf2_syntax::parse_model(source)
                    .message
                    .expect("the test message parses")
            })
            .collect();
        let messages: Vec<_> = parsed.iter().map(Some).collect();
        let bytes = writer::catalog(&manifest, &messages, &Options::new(tag, dir))
            .expect("the test catalog writes");
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let name: &'static str =
            Box::leak(format!("{tag}.{}.mf2b", mf2_catalog::content_hash(bytes)).into_boxed_str());
        locales.push((tag, dir));
        files.push(CatalogFile::new(tag, name, embed.then_some(bytes)));
        raw.push((name, bytes.to_vec()));
    }
    // Matched as a generated corpus is, over CLDR's whole table, which
    // gives these locales what the build's cut would.
    let corpus = Corpus::new(
        languages[0].0,
        manifest.hash(),
        Box::leak(locales.into_boxed_slice()),
        &REGISTRY,
        Box::leak(files.into_boxed_slice()),
    )
    .with_language_matching(mf2::LanguageMatching::cldr());
    // With dates, the host that resolves a named zone, as a generated
    // corpus names it (`Corpus::with_host`).
    #[cfg(feature = "fn-datetime")]
    let corpus = corpus.with_host(&mf2::host_std::ZONES_HOST);
    (Box::leak(Box::new(corpus)), raw)
}

/// A directory holding catalog files, removed when dropped.
pub(crate) struct TempDir(pub(crate) std::path::PathBuf);

impl TempDir {
    /// A fresh directory under the system's temporary one, holding `files`.
    pub(crate) fn with(label: &str, files: &[(&str, &[u8])]) -> TempDir {
        let root = std::env::temp_dir().join(format!("mf2-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the temporary directory is created");
        for (name, bytes) in files {
            std::fs::write(root.join(name), bytes).expect("the catalog is written");
        }
        TempDir(root)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
