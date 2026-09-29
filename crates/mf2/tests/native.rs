//! A native application's catalogs — embedded, or read from files checked
//! against their names — and its locale (`mf2::native`; 1.x's `mf2-native`,
//! whose tests these were).

use mf2::native::{LocaleOrigin, NativeError, NativeI18n};
use mf2::{
    BidiStrategy, CatalogFile, Compiled, Corpus, Dir, ErrorSink, Formatter, Message, PartSink,
    Registry, Sink,
};

static REGISTRY: Registry = Registry::EMPTY;

/// The one message of a `compile_str` catalog.
struct Only;

impl Message for Only {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        f.write(Compiled::ID, &[], out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        f.parts(Compiled::ID, &[], out, errs);
    }
}

/// One catalog per `(tag, message)`, compiled against one manifest, under
/// the `<tag>.<content-hash>.mf2b` name the build gives it.
fn corpus(
    messages: &[(&'static str, &str)],
    embed: bool,
) -> (&'static Corpus, Vec<(&'static str, Vec<u8>)>) {
    let mut hash = None;
    let mut locales = Vec::new();
    let mut files = Vec::new();
    let mut raw = Vec::new();
    for &(tag, source) in messages {
        let compiled = mf2::compile_str(source, tag).expect("the test message compiles");
        let h = compiled.catalog.manifest_hash();
        assert_eq!(*hash.get_or_insert(h), h, "one manifest");
        let bytes: &'static [u8] = Box::leak(compiled.catalog.as_bytes().to_vec().into());
        let name: &'static str =
            Box::leak(format!("{tag}.{}.mf2b", mf2_catalog::content_hash(bytes)).into_boxed_str());
        locales.push((tag, Dir::Ltr));
        files.push(CatalogFile::new(tag, name, embed.then_some(bytes)));
        raw.push((name, bytes.to_vec()));
    }
    let corpus = Corpus::new(
        messages[0].0,
        hash.unwrap_or_default(),
        Box::leak(locales.into()),
        &REGISTRY,
        Box::leak(files.into()),
    );
    (Box::leak(Box::new(corpus)), raw)
}

#[test]
fn embedded_catalogs_format_and_switch_locale() {
    let (corpus, raw) = corpus(&[("en", "Welcome"), ("fr", "Bienvenue")], true);
    let mut i18n = NativeI18n::embedded(corpus).expect("embedded catalogs load");

    i18n.set_locale("FR_ca").expect("fr-CA falls back to fr");
    assert_eq!(i18n.locale(), "fr");
    assert_eq!(i18n.locale_source(), LocaleOrigin::Explicit);
    assert_eq!(i18n.format(&Only), "Bienvenue");

    let en = format!("en.{}.mf2b", mf2_catalog::content_hash(&raw[0].1));
    assert_eq!(i18n.catalog_file_name("en"), Some(en.as_str()));
    assert_eq!(i18n.available_locales().collect::<Vec<_>>(), ["en", "fr"]);
}

#[test]
fn an_unsupported_locale_is_an_error_and_changes_nothing() {
    let (corpus, _) = corpus(&[("en", "Welcome")], true);
    let mut i18n = NativeI18n::embedded(corpus).expect("embedded catalog loads");
    let before = (i18n.locale(), i18n.locale_source());

    assert!(matches!(
        i18n.set_locale("xx"),
        Err(NativeError::UnknownLocale(tag)) if tag == "xx"
    ));
    assert_eq!((i18n.locale(), i18n.locale_source()), before);
}

#[test]
fn placeholders_are_not_isolated_unless_asked() {
    let (corpus, _) = corpus(&[("en", ".local $x = {|Bob|} {{Hi {$x}}}")], true);
    let mut i18n = NativeI18n::embedded(corpus).expect("embedded catalog loads");
    assert_eq!(i18n.bidi(), BidiStrategy::None);
    assert_eq!(i18n.format(&Only), "Hi Bob");

    i18n.set_bidi(BidiStrategy::Default);
    assert_eq!(i18n.format(&Only), "Hi \u{2068}Bob\u{2069}");
}

#[test]
fn a_files_corpus_is_not_embedded() {
    let (corpus, _) = corpus(&[("en", "Welcome")], false);
    assert!(matches!(
        NativeI18n::embedded(corpus),
        Err(NativeError::NotEmbedded(tag)) if tag == "en"
    ));
}

#[test]
fn external_catalogs_load_from_a_directory() {
    let (corpus, raw) = corpus(&[("en", "Welcome")], false);
    let root = std::env::temp_dir().join(format!("mf2-native-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("temporary directory is created");
    for (name, bytes) in &raw {
        std::fs::write(root.join(name), bytes).expect("catalog is written");
    }

    let loaded = NativeI18n::from_directory(corpus, &root);
    let missing = NativeI18n::from_directory(corpus, root.join("nowhere"));
    std::fs::remove_dir_all(&root).expect("temporary directory is removed");

    assert_eq!(loaded.expect("catalog loads").format(&Only), "Welcome");
    assert!(matches!(missing, Err(NativeError::Io { .. })));
}

/// A temporary directory holding catalog files, removed when dropped.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn with(label: &str, files: &[(&str, &[u8])]) -> TempDir {
        let root = std::env::temp_dir().join(format!("mf2-native-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary directory is created");
        for (name, bytes) in files {
            std::fs::write(root.join(name), bytes).expect("catalog is written");
        }
        TempDir(root)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A rebuild that changes only a message's text keeps the manifest hash, so
/// the reader alone cannot tell the two builds' catalogs apart. The old file
/// renamed to the new build's name must still be refused: its bytes do not
/// give the hash in that name.
#[test]
fn a_renamed_catalog_from_a_text_only_rebuild_is_refused() {
    let (old, old_raw) = corpus(&[("en", "Welcome")], false);
    let (new, new_raw) = corpus(&[("en", "Welcome back")], false);
    assert_eq!(
        old.manifest_hash(),
        new.manifest_hash(),
        "a text-only rebuild"
    );
    let (new_name, new_bytes) = &new_raw[0];
    let (old_name, old_bytes) = &old_raw[0];
    assert_ne!(new_name, old_name);

    // The new build's catalog, under its own name, loads.
    let fresh = TempDir::with("fresh", &[(*new_name, new_bytes.as_slice())]);
    let loaded = NativeI18n::from_directory(new, &fresh.0).expect("the new build's catalog loads");
    assert_eq!(loaded.format(&Only), "Welcome back");

    // The old build's catalog, renamed to the new build's name, does not.
    let renamed = TempDir::with("renamed", &[(*new_name, old_bytes.as_slice())]);
    let refused = NativeI18n::from_directory(new, &renamed.0);
    let old_hash = mf2_catalog::content_hash(old_bytes);
    match refused {
        Err(NativeError::ContentMismatch { path, actual }) => {
            assert!(path.ends_with(new_name), "the path names the file read");
            assert_eq!(actual, old_hash, "the hash of the bytes read");
        }
        Err(other) => panic!("refused, but as {other:?}"),
        Ok(i18n) => panic!(
            "a catalog from another build loaded and prints {:?}",
            i18n.format(&Only)
        ),
    }
}

#[test]
fn a_catalog_for_another_locale_is_rejected() {
    let (good, _) = corpus(&[("en", "Welcome")], true);
    let bytes = good.catalogs()[0].bytes();
    let files: &'static [CatalogFile] =
        Box::leak(Box::new([CatalogFile::new("de", "de.mf2b", bytes)]));
    let wrong = Box::leak(Box::new(Corpus::new(
        "de",
        good.manifest_hash(),
        &[("de", Dir::Ltr)],
        &REGISTRY,
        files,
    )));
    assert!(matches!(
        NativeI18n::embedded(wrong),
        Err(NativeError::LocaleMismatch { expected, actual }) if expected == "de" && actual == "en"
    ));
}
