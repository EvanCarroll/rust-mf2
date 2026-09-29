//! The one lookup with a Leptos mode and `native` both on, as a workspace
//! that holds a web application and a native one builds `mf2` on the host
//! (plans/19-native-and-terminal.md §3, §5): the request's catalog (`ssr`)
//! or the page's (`hydrate`, `csr`) first, then the native thread's
//! language, then the app-wide one; before any of them, the web's rule — no
//! text, never a panic. Nothing here installs the native store, so outside
//! `with_locale` it has no language.
//!
//! `cargo test -p mf2 --features leptos,ssr,native,compile --test lookup`,
//! and the same with `csr`.
#![cfg(any(feature = "ssr", feature = "hydrate", feature = "csr"))]

// The Leptos line under test: `mf2` names each line under a name of its
// own, and a test binds the one that is on.
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos")))]
extern crate leptos_0_8 as leptos;
#[cfg(feature = "leptos")]
extern crate leptos_0_9 as leptos;

#[path = "support/corpus.rs"]
mod support;

use std::sync::{Arc, OnceLock};

use mf2::leptos::{Setup, install};
use mf2::native;
use mf2::{Catalog, Corpus, Dir};
use support::{files, welcome};

/// The native store's corpus.
fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<&'static Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| support::corpus(true).0)
}

/// The web side's catalog: the same messages, other text, so that which
/// side answered shows.
fn web_catalog() -> Arc<Catalog> {
    let (corpus, raw) = support::corpus_of(
        &[(
            "en",
            Dir::Ltr,
            [
                "Welcome (web)",
                "Hello, {$name} (web)",
                "{$n :integer} files (web)",
            ],
        )],
        true,
    );
    install(Setup::new(
        &support::REGISTRY,
        &mf2::host_std::HOST,
        corpus.manifest_hash(),
        "en",
        &[("en", Dir::Ltr)],
    ));
    Arc::new(Catalog::new(raw[0].1.clone(), corpus.manifest_hash()).expect("the catalog reads"))
}

/// Runs `body` with the web side's catalog in force: in a request, on the
/// server; as the page's, on the client (this thread's).
fn with_web<R>(body: impl FnOnce() -> R) -> R {
    let catalog = web_catalog();
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::Owner;
        let owner = Owner::new();
        owner.with(|| {
            mf2::leptos::RequestI18n::new(catalog).provide();
            body()
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        mf2::leptos::set_active(catalog);
        body()
    }
}

#[test]
fn the_request_s_or_the_page_s_catalog_comes_first() {
    let shown = with_web(|| {
        native::with_locale(corpus(), "fr", || {
            (
                welcome().to_string(),
                format!("{}", files(3)),
                welcome().to_cow().into_owned(),
            )
        })
        .expect("fr is supported")
    });
    assert_eq!(shown.0, "Welcome (web)");
    assert_eq!(shown.1, "3 files (web)");
    assert_eq!(shown.2, "Welcome (web)");
}

#[test]
fn without_it_the_native_thread_s_language_answers() {
    // A fresh thread: on the client the page's catalog is the thread's.
    std::thread::spawn(|| {
        native::with_locale(corpus(), "fr", || {
            assert_eq!(welcome().to_string(), "Bienvenue");
            assert_eq!(format!("{}", files(3)), "3 fichiers");
            assert!(matches!(
                welcome().to_cow(),
                std::borrow::Cow::Borrowed("Bienvenue")
            ));
        })
        .expect("fr is supported");
    })
    .join()
    .expect("the store answers");
}

#[test]
fn with_neither_the_web_s_rule_holds() {
    std::thread::spawn(|| {
        // No request, no page catalog, no native language: no text, and no
        // panic, as the web's rule says.
        assert_eq!(welcome().to_string(), "");
        assert_eq!(format!("[{}]", files(3)), "[]");
    })
    .join()
    .expect("never a panic beside a Leptos mode");
}
