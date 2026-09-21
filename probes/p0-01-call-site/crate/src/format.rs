//! Stub formatter: catalog text, then each argument after a space. fmt-free:
//! integers are converted by hand, floats are truncated (number formatting
//! is P0.5's subject, not this probe's).

use std::cell::RefCell;

use leptos::prelude::{Get, GetUntracked};

use crate::MsgId;
use crate::args::ArgValue;
use crate::catalog;

/// Whether reading reactive arguments subscribes the current observer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tracking {
    Tracked,
    Untracked,
}

thread_local! {
    /// Shared scratch buffer for pattern formatting (plans/06 §4, item 12).
    static SCRATCH: RefCell<String> = const { RefCell::new(String::new()) };
}

fn push_i64(out: &mut String, v: i64) {
    let mut digits = [0u8; 20];
    let mut n = v.unsigned_abs();
    let mut len = 0usize;
    loop {
        if let Some(d) = digits.get_mut(len) {
            #[allow(clippy::cast_possible_truncation)]
            let digit = (n % 10) as u8;
            *d = b'0' + digit;
        }
        len += 1;
        n /= 10;
        if n == 0 || len >= digits.len() {
            break;
        }
    }
    if v < 0 {
        out.push('-');
    }
    for &d in digits.get(..len).unwrap_or(&[]).iter().rev() {
        out.push(char::from(d));
    }
}

fn push_plain(out: &mut String, v: &ArgValue) {
    match v {
        ArgValue::Str(s) => out.push_str(s),
        ArgValue::Int(i) => push_i64(out, *i),
        #[allow(clippy::cast_possible_truncation)]
        ArgValue::Float(f) => push_i64(out, *f as i64),
        // One level only: a signal of a signal is not a call-site value.
        ArgValue::Reactive(_) => {}
    }
}

/// Appends the formatted message to `out`.
pub(crate) fn format_into(
    out: &mut String,
    text: &str,
    args: &[ArgValue],
    tracking: Tracking,
) {
    out.push_str(text);
    for a in args {
        out.push(' ');
        match a {
            ArgValue::Reactive(sig) => {
                let v = match tracking {
                    Tracking::Tracked => sig.get(),
                    Tracking::Untracked => sig.get_untracked(),
                };
                push_plain(out, &v);
            }
            other => push_plain(out, other),
        }
    }
}

/// Calls `f` with the formatted text: the catalog's `&str` directly for a
/// message without arguments (no `String`), the scratch buffer otherwise.
#[inline(never)]
pub(crate) fn with_resolved(
    id: MsgId,
    args: &[ArgValue],
    tracking: Tracking,
    f: &mut dyn FnMut(&str),
) {
    let cat = catalog::active();
    let text = cat.as_deref().map_or("", |c| c.get(id));
    if args.is_empty() {
        f(text);
        return;
    }
    let mut buf = SCRATCH
        .try_with(|s| s.try_borrow_mut().map(|mut s| core::mem::take(&mut *s)).ok())
        .ok()
        .flatten()
        .unwrap_or_default();
    buf.clear();
    format_into(&mut buf, text, args, tracking);
    f(&buf);
    let _ = SCRATCH.try_with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            *s = buf;
        }
    });
}

/// The formatted message as an owned `String` (one allocation).
#[inline(never)]
pub(crate) fn resolve_string(id: MsgId, args: &[ArgValue], tracking: Tracking) -> String {
    let cat = catalog::active();
    let text = cat.as_deref().map_or("", |c| c.get(id));
    let mut out = String::with_capacity(text.len() + 8 * args.len());
    format_into(&mut out, text, args, tracking);
    out
}

/// Server rendering: the request catalog's text, formatted if needed.
#[inline(never)]
pub(crate) fn with_html_resolved(id: MsgId, args: &[ArgValue], f: &mut dyn FnMut(&str)) {
    catalog::with_html_text(id, |text| {
        if args.is_empty() {
            f(text);
        } else {
            let mut buf = String::with_capacity(text.len() + 8 * args.len());
            format_into(&mut buf, text, args, Tracking::Tracked);
            f(&buf);
        }
    });
}

#[cfg_attr(any(feature = "effect", feature = "static-locale"), allow(dead_code))]
pub(crate) fn has_reactive(args: &[ArgValue]) -> bool {
    args.iter().any(ArgValue::is_reactive)
}
