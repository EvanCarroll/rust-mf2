//! The server's one-time warnings: what used to fall back
//! without a word says so on standard error, once per process for each kind
//! (and, where it helps, each language), never once per request.
//!
//! Standard error rather than a logging facade: a warning that only shows
//! when the application has installed a subscriber would stay as silent as
//! the fallback it reports.

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

/// What went unsaid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    not(all(feature = "axum", feature = "ssr")),
    allow(
        dead_code,
        reason = "a server without Leptos, or Leptos without `axum`, gives some"
    )
)]
pub(crate) enum Kind {
    /// A message formatted with no catalogs installed: empty text.
    NoCatalogs = 0,
    /// A page rendered with no language for its request: the source
    /// language's text.
    Unrequested = 1,
    /// `provide_locale` named a language the build has no catalog for.
    UnknownLocale = 2,
    /// The reader named languages and no catalog matches them.
    Unmatched = 3,
    /// `path_prefix_redirect` ran outside the negotiator.
    RedirectOutside = 4,
}

/// The kinds with no key: one flag each, read before anything else so that
/// the hot path costs one load once the warning has been given.
static DONE: [AtomicBool; 5] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];

/// The keys already warned about, per kind.
static SEEN: Mutex<Vec<(Kind, String)>> = Mutex::new(Vec::new());

/// At most this many keys per kind: a key can come from a request header,
/// and a stream of made-up languages must not grow the set without end.
const KEYS_PER_KIND: usize = 32;

/// Whether a keyless warning of `kind` is still to be given — for a caller
/// that would otherwise do work to find out whether to call [`once`].
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
pub(crate) fn pending(kind: Kind) -> bool {
    !flag(kind).load(Ordering::Relaxed)
}

fn flag(kind: Kind) -> &'static AtomicBool {
    match kind {
        Kind::NoCatalogs => &DONE[0],
        Kind::Unrequested => &DONE[1],
        Kind::UnknownLocale => &DONE[2],
        Kind::Unmatched => &DONE[3],
        Kind::RedirectOutside => &DONE[4],
    }
}

/// Gives the warning of `kind` once per process.
pub(crate) fn once(kind: Kind, message: impl FnOnce() -> String) {
    if !flag(kind).swap(true, Ordering::Relaxed) {
        emit(kind, &message());
    }
}

/// Gives the warning of `kind` once per process for `key`.
pub(crate) fn once_for(kind: Kind, key: &str, message: impl FnOnce() -> String) {
    {
        let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
        if seen.iter().any(|(k, s)| *k == kind && s == key)
            || seen.iter().filter(|(k, _)| *k == kind).count() >= KEYS_PER_KIND
        {
            return;
        }
        seen.push((kind, String::from(key)));
    }
    emit(kind, &message());
}

/// `text` if it can be a language tag, for a warning to name: letters,
/// digits, `-` and `_`, not too long. What a request sent is anything.
pub(crate) fn tag(text: &str) -> Option<&str> {
    (!text.is_empty()
        && text.len() <= 35
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then_some(text)
}

fn emit(kind: Kind, line: &str) {
    std::eprintln!("{line}");
    #[cfg(test)]
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((kind, String::from(line)));
    #[cfg(not(test))]
    let _ = kind;
}

/// Every warning given in this test process.
#[cfg(test)]
static LOG: Mutex<Vec<(Kind, String)>> = Mutex::new(Vec::new());

/// Held by each test that counts the warnings of a kind. `SEEN` and `LOG`
/// belong to the process, and the unit tests of this crate share one: without
/// this, a test that fills a kind's key budget silences another test's
/// warning, or is credited with its line, depending on the thread order.
#[cfg(test)]
static COUNTING: Mutex<()> = Mutex::new(());

/// Takes the turn of the tests that count warnings; held for as long as the
/// returned guard lives.
#[cfg(test)]
pub(crate) fn counting() -> std::sync::MutexGuard<'static, ()> {
    COUNTING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Forgets the warnings of `kind`, keys and lines both, so that a test which
/// fills that kind's key budget leaves it as it found it. Only the keys and
/// lines of `kind` go: another kind's test may be running. Test-only; the
/// budget an application sees is never reset.
#[cfg(test)]
pub(crate) fn forget(kind: Kind) {
    SEEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(k, _)| *k != kind);
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(k, _)| *k != kind);
}

/// The warnings of `kind` given so far in this test process.
#[cfg(test)]
pub(crate) fn given(kind: Kind) -> Vec<String> {
    LOG.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|(k, _)| *k == kind)
        .map(|(_, line)| line.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Kind, counting, forget, given, once_for, tag};
    use alloc::string::String;

    #[test]
    fn a_keyed_warning_is_given_once_per_key_and_the_keys_are_bounded() {
        // This test fills the key budget of its kind, so it takes the turn of
        // the tests that count warnings and hands the kind back empty.
        let _counting = counting();
        forget(Kind::UnknownLocale);
        for _ in 0..3 {
            once_for(Kind::UnknownLocale, "warn-test-a", || {
                String::from("warn-test-a")
            });
            once_for(Kind::UnknownLocale, "warn-test-b", || {
                String::from("warn-test-b")
            });
        }
        let lines = given(Kind::UnknownLocale);
        assert_eq!(lines.iter().filter(|l| *l == "warn-test-a").count(), 1);
        assert_eq!(lines.iter().filter(|l| *l == "warn-test-b").count(), 1);
        for n in 0..100 {
            let key = alloc::format!("warn-test-many-{n}");
            once_for(Kind::UnknownLocale, &key, || key.clone());
        }
        assert!(given(Kind::UnknownLocale).len() <= super::KEYS_PER_KIND);
        forget(Kind::UnknownLocale);
    }

    #[test]
    fn only_tag_shaped_text_is_named() {
        assert_eq!(tag("zh-Hant-TW"), Some("zh-Hant-TW"));
        assert_eq!(tag("*"), None);
        assert_eq!(tag("en\n[forged]"), None);
        assert_eq!(tag(""), None);
    }
}
