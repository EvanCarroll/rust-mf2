//! The native store (plans/19-native-and-terminal.md §5): `install`, the
//! app-wide language and a thread's own, the settings, and the ambient forms
//! — `Display`, `to_string()`, `to_plain_string()`, `to_cow()` — read
//! through it; and `Catalogs`, the explicit form.
//!
//! The store is process-wide: every test here installs one corpus, the same
//! one, and the tests that change the app-wide language or the settings, or
//! read them, hold one lock. With a Leptos mode beside `native` (the
//! workspace's build unifies `ssr`), nothing here is in a request, so the
//! lookup's first step finds nothing and the store answers.

#[path = "support/corpus.rs"]
mod support;

use std::borrow::Cow;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError, mpsc};

use mf2::native::{self, Catalogs, Error, LocaleSource};
use mf2::{BidiStrategy, Corpus, TimeZone};
use support::{TempDir, files, hello, welcome};

/// The one corpus of this process, installed.
fn installed() -> &'static Corpus {
    static CORPUS: OnceLock<&'static Corpus> = OnceLock::new();
    let corpus = *CORPUS.get_or_init(|| support::corpus(true).0);
    native::install(corpus);
    corpus
}

/// Held by each test that changes the app-wide language or the settings, or
/// reads them.
fn app_wide() -> MutexGuard<'static, ()> {
    static APP_WIDE: Mutex<()> = Mutex::new(());
    APP_WIDE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a panic said.
fn said(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or_default()
}

#[test]
fn install_is_idempotent_and_refuses_another_corpus() {
    let corpus = installed();
    native::install(corpus);
    let (other, _) = support::corpus(true);
    let refused = std::panic::catch_unwind(AssertUnwindSafe(|| native::install(other)))
        .expect_err("another corpus is refused");
    assert!(
        said(&*refused).starts_with("mf2: install(): another corpus is already installed"),
        "{}",
        said(&*refused)
    );
    assert!(matches!(
        native::with_locale(other, "fr", || ()),
        Err(Error::AnotherCorpus)
    ));
    // The store still holds the first.
    native::with_locale(corpus, "fr", || assert_eq!(welcome().to_cow(), "Bienvenue"))
        .expect("fr is supported");
}

#[test]
fn a_simple_message_is_borrowed_from_the_executable() {
    let corpus = installed();
    native::with_locale(corpus, "fr", || {
        let text = welcome().to_cow();
        let Cow::Borrowed(borrowed) = text else {
            panic!("a simple message is borrowed, not {text:?}");
        };
        assert_eq!(borrowed, "Bienvenue");
        let bytes = corpus.catalogs()[1]
            .bytes()
            .expect("the catalog is embedded");
        assert!(
            bytes.as_ptr_range().contains(&borrowed.as_ptr()),
            "the text is the embedded catalog's own bytes"
        );
        // Anything else is formatted, and owned.
        let formatted = files(3).to_cow();
        assert!(matches!(formatted, Cow::Owned(_)), "{formatted:?}");
        assert_eq!(formatted, "3 fichiers");
        // `Display` is the same text, padded as the format asks.
        assert_eq!(format!("{}", welcome()), "Bienvenue");
        assert_eq!(format!("[{:<12}]", welcome()), "[Bienvenue   ]");
        assert_eq!(format!("[{:>12}]", files(3)), "[  3 fichiers]");
        assert_eq!(welcome().to_string(), "Bienvenue");
        assert_eq!(String::from(welcome()), "Bienvenue");
        assert_eq!(files(2).to_plain_string(), "2 fichiers");
    })
    .expect("fr is supported");
}

#[test]
fn pinned_threads_format_in_their_own_language_in_parallel() {
    let corpus = installed();
    let _app_wide = app_wide();
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        // Meanwhile the app-wide language and settings keep changing.
        s.spawn(|| {
            let zones = [
                TimeZone::UTC,
                TimeZone::named("Europe/Paris").expect("a zone"),
            ];
            let mut i = 0_usize;
            while !stop.load(Ordering::Relaxed) {
                native::set_locale(["en", "fr"][i % 2]).expect("supported");
                native::set_bidi([BidiStrategy::Default, BidiStrategy::None][i % 2]);
                native::set_time_zone(zones[i % 2]);
                i += 1;
                std::thread::yield_now();
            }
        });
        let pinned: Vec<_> = (0..16)
            .map(|t| {
                let (tag, simple, number, greeting) = if t % 2 == 0 {
                    ("en", "Welcome", "2 files", "Hello, Ada!")
                } else {
                    ("fr", "Bienvenue", "2 fichiers", "Bonjour, Ada !")
                };
                s.spawn(move || {
                    native::with_locale(corpus, tag, || {
                        for _ in 0..2_000 {
                            assert_eq!(native::locale(), tag);
                            assert_eq!(welcome().to_cow(), simple);
                            assert_eq!(format!("{}", welcome()), simple);
                            assert_eq!(files(2).to_string(), number);
                            assert_eq!(hello("Ada").to_plain_string(), greeting);
                        }
                    })
                    .expect("supported");
                })
            })
            .collect();
        for thread in pinned {
            thread
                .join()
                .expect("each pinned thread formats in its own language");
        }
        stop.store(true, Ordering::Relaxed);
    });
}

#[test]
fn set_locale_is_seen_by_another_thread_on_its_next_format() {
    installed();
    let _app_wide = app_wide();
    let (ask, asked) = mpsc::channel::<()>();
    let (answer, answers) = mpsc::channel::<String>();
    std::thread::scope(|s| {
        s.spawn(move || {
            for () in asked {
                answer
                    .send(welcome().to_string())
                    .expect("the test listens");
            }
        });
        let next = || {
            ask.send(()).expect("the worker listens");
            answers.recv().expect("the worker answers")
        };
        native::set_locale("fr").expect("fr is supported");
        assert_eq!(next(), "Bienvenue");
        assert_eq!(native::locale_source(), LocaleSource::Explicit);
        native::set_locale("en_GB.UTF-8").expect("en-GB falls back to en");
        assert_eq!(next(), "Welcome");
        assert_eq!(native::locale(), "en");
        assert!(matches!(
            native::set_locale("de"),
            Err(Error::UnknownLocale(tag)) if tag == "de"
        ));
        assert_eq!(next(), "Welcome", "an unsupported locale changes nothing");
        drop(ask);
    });
}

#[test]
fn with_locale_nests_and_is_restored_when_its_body_unwinds() {
    let corpus = installed();
    native::with_locale(corpus, "fr", || {
        assert_eq!(native::locale(), "fr");
        native::with_locale(corpus, "EN", || {
            assert_eq!(native::locale(), "en");
            assert_eq!(welcome().to_cow(), "Welcome");
        })
        .expect("en is supported");
        assert_eq!(native::locale(), "fr");
        let unwound = std::panic::catch_unwind(AssertUnwindSafe(|| {
            native::with_locale(corpus, "en", || panic!("inside with_locale"))
        }));
        assert_eq!(
            said(&*unwound.expect_err("the body panicked")),
            "inside with_locale"
        );
        assert_eq!(native::locale(), "fr", "restored as the body unwound");
        assert_eq!(welcome().to_cow(), "Bienvenue");
    })
    .expect("fr is supported");
    assert!(matches!(
        native::with_locale(corpus, "de", || ()),
        Err(Error::UnknownLocale(tag)) if tag == "de"
    ));
}

#[test]
fn the_settings_reach_every_thread_on_its_next_format() {
    installed();
    let _app_wide = app_wide();
    native::set_locale("en").expect("en is supported");
    let (ask, asked) = mpsc::channel::<()>();
    let (answer, answers) = mpsc::channel();
    std::thread::scope(|s| {
        s.spawn(move || {
            for () in asked {
                let shown = (
                    hello("Ada").to_string(),
                    hello("Ada").to_plain_string(),
                    native::bidi(),
                    native::time_zone(),
                );
                answer.send(shown).expect("the test listens");
            }
        });
        let next = || {
            ask.send(()).expect("the worker listens");
            answers.recv().expect("the worker answers")
        };
        native::set_bidi(BidiStrategy::None);
        let (text, plain, bidi, _) = next();
        assert_eq!(
            (text.as_str(), plain.as_str()),
            ("Hello, Ada!", "Hello, Ada!")
        );
        assert_eq!(bidi, BidiStrategy::None);
        native::set_bidi(BidiStrategy::Default);
        let (text, plain, bidi, _) = next();
        assert_eq!(
            text, "Hello, \u{2068}Ada\u{2069}!",
            "isolated as the setting says"
        );
        assert_eq!(plain, "Hello, Ada!", "never isolated");
        assert_eq!(bidi, BidiStrategy::Default);
        let tokyo = TimeZone::named("Asia/Tokyo").expect("a zone");
        native::set_time_zone(tokyo);
        assert_eq!(next().3, tokyo);
        native::set_bidi(BidiStrategy::None);
        drop(ask);
    });
}

#[test]
fn catalogs_format_in_the_language_named_with_no_globals() {
    let (corpus, raw) = support::corpus(true);
    let mut catalogs = Catalogs::embedded(corpus).expect("the catalogs load");
    assert_eq!(catalogs.locales().collect::<Vec<_>>(), ["en", "fr"]);
    assert_eq!(catalogs.format("fr_CA.UTF-8", &welcome()), "Bienvenue");
    assert_eq!(catalogs.format("en", &files(3)), "3 files");
    assert_eq!(
        catalogs.format("de", &welcome()),
        "Welcome",
        "an unsupported language is the source's"
    );
    assert_eq!(catalogs.bidi(), BidiStrategy::None);
    assert_eq!(catalogs.format("en", &hello("Ada")), "Hello, Ada!");
    catalogs.set_bidi(BidiStrategy::Default);
    assert_eq!(
        catalogs.format("en", &hello("Ada")),
        "Hello, \u{2068}Ada\u{2069}!"
    );

    // From a directory, a partial set: only the source language's file is
    // required, and a language whose file is absent is not offered.
    let (files_corpus, _) = support::corpus(false);
    let (name, bytes) = &raw[0];
    let partial = TempDir::with("ambient-partial", &[(name, bytes)]);
    let loaded =
        Catalogs::from_directory(files_corpus, &partial.0).expect("the source's is enough");
    assert_eq!(loaded.locales().collect::<Vec<_>>(), ["en"]);
    assert_eq!(loaded.format("fr", &welcome()), "Welcome");
    let empty = TempDir::with("ambient-empty", &[]);
    assert!(matches!(
        Catalogs::from_directory(files_corpus, &empty.0),
        Err(Error::Io { .. })
    ));
}
