//! Before `install()`: in a build whose
//! only mode is `native`, the app-wide forms panic and name `install()`; in a
//! build with a Leptos mode beside it, the web's rule holds — no text, never
//! a panic. `with_locale` and the settings need no `install()`.
//!
//! A test binary of its own: nothing here installs, so the store stays
//! without an app-wide language for the whole process.

#[path = "support/corpus.rs"]
mod support;

use std::sync::OnceLock;

use mf2::native;
use mf2::{BidiStrategy, Corpus};
use support::{files, welcome};

/// Whether a Leptos mode is on beside `native` (the workspace's build
/// unifies `ssr`).
const WEB: bool = cfg!(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
));

const NOT_INSTALLED: &str = "mf2: no catalogs are installed: call install() at start-up";

/// The one corpus of this process, never installed.
fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<&'static Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| support::corpus(true).0)
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
fn the_ambient_forms_before_install_panic_naming_it_or_follow_the_web() {
    for form in [
        (|| welcome().to_string()) as fn() -> String,
        || files(3).to_plain_string(),
        || welcome().to_cow().into_owned(),
        || format!("{}", welcome()),
    ] {
        let shown = std::panic::catch_unwind(form);
        if WEB {
            assert_eq!(shown.expect("never a panic beside a Leptos mode"), "");
        } else {
            assert_eq!(
                said(&*shown.expect_err("a native-only build panics")),
                NOT_INSTALLED
            );
        }
    }
    for app_wide in [
        || {
            let _ = native::locale();
        },
        || {
            let _ = native::locale_source();
        },
        || {
            let _ = native::set_locale("fr");
        },
    ] {
        let refused =
            std::panic::catch_unwind(app_wide).expect_err("the app-wide forms need install()");
        assert_eq!(said(&*refused), NOT_INSTALLED);
    }
}

#[test]
fn with_locale_needs_no_install() {
    native::with_locale(corpus(), "fr", || {
        assert_eq!(native::locale(), "fr");
        assert_eq!(welcome().to_cow(), "Bienvenue");
        assert_eq!(files(3).to_string(), "3 fichiers");
    })
    .expect("fr is supported");
    // Still no app-wide language outside it.
    if !WEB {
        assert!(std::panic::catch_unwind(|| welcome().to_string()).is_err());
    }
}

#[test]
fn the_settings_need_no_install() {
    native::set_bidi(BidiStrategy::Default);
    assert_eq!(native::bidi(), BidiStrategy::Default);
    native::set_bidi(BidiStrategy::None);
    assert_eq!(native::bidi(), BidiStrategy::None);
}
