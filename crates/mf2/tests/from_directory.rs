//! `install_from_directory`: catalogs
//! shipped beside the executable, each checked against its content-hashed
//! name, and a partial set accepted — only the source language's file is
//! required. A test binary of its own, since it installs the process's
//! store.

#[path = "support/corpus.rs"]
mod support;

use mf2::Dir;
use mf2::native::{self, Error, LocaleSource};
use support::{TempDir, files, welcome};

#[test]
fn a_partial_set_installs_and_another_build_s_file_is_refused() {
    let (corpus, raw) = support::corpus(false);
    let [(en_name, en), (fr_name, _)] = &raw[..] else {
        panic!("two languages");
    };

    // Nothing shipped: the source language's file is missing.
    let empty = TempDir::with("from-directory-empty", &[]);
    assert!(matches!(
        native::install_from_directory(corpus, &empty.0),
        Err(Error::Io { .. })
    ));

    // French from another build under this build's name: refused.
    let (_, other) = support::corpus_of(
        &[
            ("en", Dir::Ltr, support::LANGUAGES[0].2),
            (
                "fr",
                Dir::Ltr,
                ["Salut", "Bonjour, {$name} !", "{$n :integer} fichiers"],
            ),
        ],
        false,
    );
    let skewed = TempDir::with(
        "from-directory-skewed",
        &[(en_name, en), (fr_name, &other[1].1)],
    );
    assert!(matches!(
        native::install_from_directory(corpus, &skewed.0),
        Err(Error::ContentMismatch { .. })
    ));

    // The source language's alone: installed, and French is not offered.
    let partial = TempDir::with("from-directory-partial", &[(en_name, en)]);
    native::install_from_directory(corpus, &partial.0).expect("the source's file is enough");
    assert_eq!(native::locale(), "en");
    assert!(matches!(
        native::locale_source(),
        LocaleSource::System | LocaleSource::Source
    ));
    assert!(matches!(
        native::set_locale("fr"),
        Err(Error::UnknownLocale(tag)) if tag == "fr"
    ));
    assert_eq!(welcome().to_string(), "Welcome");
    assert_eq!(format!("{}", files(3)), "3 files");

    // Installed: a second call reads nothing, and `install` finds the same
    // corpus in the store.
    native::install_from_directory(corpus, &empty.0).expect("already installed");
    native::install(corpus);
    assert_eq!(native::locale(), "en");
}
