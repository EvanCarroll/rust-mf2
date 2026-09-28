//! The ambient store under threads: `with_locale` pins one thread while
//! others format concurrently in other locales, and a `set_locale` or a
//! settings change is seen by another thread's next format. These tests run
//! in parallel with each other on purpose: the store is process-wide.

use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::thread;

use ambient::{Method, Show};
use mf2::{BidiStrategy, TimeZone};
use probe_i18n::tr;

fn install() {
    ambient::install(&probe_i18n::CORPUS).expect("the corpus installs");
    ambient::install(&probe_i18n::CORPUS).expect("installing the same corpus again is fine");
}

fn text(line: &ratatui_core::text::Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Without the bidi isolation controls, which another test may turn on.
fn plain(s: &str) -> String {
    s.chars().filter(|c| !('\u{2066}'..='\u{2069}').contains(c)).collect()
}

#[test]
fn with_locale_pins_each_thread_while_others_format_concurrently() {
    install();
    const THREADS: usize = 16;
    const ROUNDS: usize = 2_000;
    let start = Arc::new(Barrier::new(THREADS));
    let workers: Vec<_> = (0..THREADS)
        .map(|i| {
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let (tag, title, connected, round) = if i % 2 == 0 {
                    ("en", "Network trace", "Connected to example.org", "Round 42")
                } else {
                    ("fr", "Trace réseau", "Connecté à example.org", "Tour 42")
                };
                start.wait();
                ambient::with_locale(tag, || {
                    for _ in 0..ROUNDS {
                        assert_eq!(ambient::locale(), tag);
                        assert_eq!(ambient::to_cow::<false>(&tr!("app.title")), title);
                        assert_eq!(Show(&tr!("app.title")).to_string(), title);
                        assert_eq!(
                            plain(&ambient::to_string::<false>(&tr!("status.round", n = 42u32))),
                            round
                        );
                        let line = ambient::line::<false>(
                            &tr!("status.connected", host = "example.org"),
                            Method::RangePool,
                        );
                        assert_eq!(text(&line), connected);
                    }
                })
                .expect("the locale is supported");
            })
        })
        .collect();
    for w in workers {
        w.join().expect("no worker failed");
    }
}

#[test]
fn with_locale_nests_and_is_restored_after_a_panic() {
    install();
    thread::spawn(|| {
        let outer = ambient::locale();
        ambient::with_locale("fr", || {
            assert_eq!(ambient::locale(), "fr");
            ambient::with_locale("en", || assert_eq!(ambient::locale(), "en"))
                .expect("en is supported");
            assert_eq!(ambient::locale(), "fr");
            let unwound = std::panic::catch_unwind(|| {
                ambient::with_locale("en", || panic!("inside")).expect("en is supported");
            });
            assert!(unwound.is_err());
            assert_eq!(ambient::locale(), "fr");
        })
        .expect("fr is supported");
        // Back to following the app-wide locale, whatever another test set.
        let _ = outer;
        assert!(ambient::with_locale("xx", || ()).is_err());
    })
    .join()
    .expect("the thread ends");
}

#[test]
fn a_change_is_seen_by_another_threads_next_format() {
    install();
    let (to_worker, orders) = mpsc::channel::<()>();
    let (to_main, seen) = mpsc::channel::<(String, String, String)>();
    let worker = thread::spawn(move || {
        for () in orders {
            let title = ambient::to_string::<false>(&tr!("app.title"));
            let arg = ambient::to_string::<false>(&tr!("status.target", host = "example.org"));
            let zone = format!("{:?}", ambient::time_zone());
            to_main.send((title, arg, zone)).expect("main listens");
        }
    });
    let ask = || {
        to_worker.send(()).expect("the worker listens");
        seen.recv().expect("the worker answers")
    };

    ambient::set_locale("fr").expect("fr is supported");
    assert_eq!(ask().0, "Trace réseau");
    ambient::set_locale("en").expect("en is supported");
    assert_eq!(ask().0, "Network trace");

    ambient::set_bidi(BidiStrategy::Default);
    assert_eq!(ask().1, "Tracing \u{2068}example.org\u{2069}");
    ambient::set_bidi(BidiStrategy::None);
    assert_eq!(ask().1, "Tracing example.org");

    let paris = TimeZone::named("Europe/Paris").expect("a known zone");
    ambient::set_time_zone(paris);
    assert_eq!(ask().2, format!("{paris:?}"));

    drop(to_worker);
    worker.join().expect("the worker ends");
}
