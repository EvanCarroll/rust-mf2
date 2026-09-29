//! A description's text as a `String`, and `Display` — the ambient forms,
//! which read the catalog in force, in the order of the one lookup
//! (plans/19-native-and-terminal.md §5):
//!
//! 1. the request's catalog (`ssr`) or the page's (`hydrate`, `csr`);
//! 2. with `native`, this thread's language (`mf2::native::with_locale`);
//! 3. with `native`, the app-wide language (`mf2::native::install`,
//!    `set_locale`).
//!
//! Each step exists only with its feature. Before any catalog is in force a
//! build with a Leptos mode shows no text (on a server, the source
//! language's catalog), and a build whose only mode is `native` panics,
//! naming `install()`.
//!
//! One text path per description type, shared by three ways to it:
//!
//! * the inherent `to_string()`, `to_plain_string()`, `to_cow()` and
//!   `String::from` — **fmt-free**, and what browser code calls: an inherent
//!   method wins name resolution over `ToString::to_string`, so
//!   `tr.to_string()` never reaches `Display`;
//! * `Display` — `format!`, `println!`, `write!`, and anything generic over
//!   `Display` or `ToString`. It pads the text `to_cow()` gives, so
//!   `{:<12}` pads. In a browser build `{}` adds a few dozen bytes over
//!   `.to_string()` in an application that already formats with `format!`;
//!   a client that never writes one links none of it. Natively a simple
//!   message's `{}` allocates nothing.
//!
//! A build with no mode has no ambient text: nothing says which catalog to
//! read, so there is neither `to_string()` nor `Display` there.
//!
//! With `native`, every form goes through one function taking a trait
//! object that lists only what the text needs (A4): each is compiled once,
//! whatever the description's type, and none links the parts path.

use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt;

#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
use crate::leptos::state::TextUse;
#[cfg(not(feature = "native"))]
use crate::leptos::text::{self, Description};
#[cfg(feature = "native")]
use crate::native::store::TextOf;
use crate::{Tr, TrArgs, TrDyn, TrRich};
#[cfg(feature = "native")]
use mf2_runtime::{Formatter, MsgId, NoErrors, Sink};

/// The text of `description` against the catalog in force: the one text path
/// the conversions below share.
#[cfg(not(feature = "native"))]
fn ambient<D: Description>(description: &D, use_: TextUse) -> String {
    text::to_string(description, use_)
}

/// The text of `m` through the one lookup; `plain`: never isolated.
#[cfg(all(
    feature = "native",
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
fn ambient(m: &dyn TextOf, plain: bool) -> Cow<'static, str> {
    let use_ = if plain {
        TextUse::Plain
    } else {
        TextUse::Displayed
    };
    // 1. The request's catalog, or the page's.
    if let Some(text) = crate::leptos::text::requested(m, use_) {
        return Cow::Owned(text);
    }
    // 2 and 3. This thread's language, else the app-wide one; before
    // either, the web's rule.
    match crate::native::store::text(m, plain) {
        Some(text) => text,
        None => Cow::Owned(crate::leptos::text::unrequested(m, use_)),
    }
}

/// The text of `m` through the one lookup; `plain`: never isolated.
#[cfg(all(
    feature = "native",
    not(all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    ))
))]
fn ambient(m: &dyn TextOf, plain: bool) -> Cow<'static, str> {
    match crate::native::store::text(m, plain) {
        Some(text) => text,
        None => crate::native::store::not_installed(),
    }
}

/// `Display`, once for every description type: the text `to_cow()` gives,
/// padded.
#[cfg(feature = "native")]
fn display(m: &dyn TextOf, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(&ambient(m, false))
}

#[cfg(feature = "native")]
macro_rules! text_of {
    ($($ty:ty),*) => {$(
        impl TextOf for $ty {
            fn msg_id(&self) -> MsgId {
                self.id()
            }

            fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
                self.write(f, out, &mut NoErrors);
            }
        }
    )*};
}

#[cfg(feature = "native")]
text_of!(Tr, TrArgs, TrRich, TrDyn);

#[cfg(not(feature = "native"))]
macro_rules! string_conversions {
    ($ty:ty) => {
        impl $ty {
            /// The message's text against the catalog in force, **isolated**:
            /// the specification's Default Bidi Strategy, which it makes the
            /// default for a message formatted as a single string.
            /// [`to_plain_string`](Self::to_plain_string) is the form for text
            /// a program consumes.
            ///
            /// The text `Display` writes, without `core::fmt`: the leanest
            /// form in a browser build.
            #[must_use]
            #[allow(
                clippy::inherent_to_string_shadow_display,
                reason = "the fmt-free form of what `Display` writes, which browser code calls; \
                          both produce the same text"
            )]
            pub fn to_string(&self) -> String {
                ambient(self, TextUse::Displayed)
            }

            /// The same text with no bidi isolation, for a `String` a
            /// program consumes — a server function, a comparison, the
            /// clipboard, `format!` — where U+2066–U+2069 would be invisible
            /// junk.
            #[must_use]
            pub fn to_plain_string(&self) -> String {
                ambient(self, TextUse::Plain)
            }

            /// The text [`to_string`](Self::to_string) returns, as a
            /// `Cow`: owned here, where the catalog is the request's or the
            /// page's; natively a simple message is borrowed.
            #[must_use]
            pub fn to_cow(&self) -> Cow<'static, str> {
                Cow::Owned(ambient(self, TextUse::Displayed))
            }

            /// A synonym of [`to_string`](Self::to_string), kept from when
            /// `to_string()` was plain.
            #[must_use]
            pub fn to_display_string(&self) -> String {
                ambient(self, TextUse::Displayed)
            }
        }

        impl From<$ty> for String {
            fn from(description: $ty) -> String {
                ambient(&description, TextUse::Displayed)
            }
        }

        /// The text [`to_string`](Self::to_string) returns, padded when the
        /// format asks (`{:<12}`).
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.pad(&ambient(self, TextUse::Displayed))
            }
        }
    };
}

#[cfg(feature = "native")]
macro_rules! string_conversions {
    ($ty:ty) => {
        impl $ty {
            /// The message's text in the language in force (the one lookup,
            /// above), isolated as that language's side says: the Default
            /// Bidi Strategy for a request's or a page's catalog, and the
            /// store's setting natively (`mf2::native::set_bidi`), off until
            /// set. [`to_plain_string`](Self::to_plain_string) is the form
            /// for text a program consumes.
            ///
            /// The text `Display` writes, without `core::fmt`.
            ///
            /// # Panics
            ///
            /// In a build whose only mode is `native`, before
            /// `mf2::native::install` (outside `with_locale`).
            #[must_use]
            #[allow(
                clippy::inherent_to_string_shadow_display,
                reason = "the fmt-free form of what `Display` writes, one allocation where \
                          `ToString` makes two; both produce the same text"
            )]
            pub fn to_string(&self) -> String {
                ambient(self, false).into_owned()
            }

            /// The same text with no bidi isolation, for a `String` a
            /// program consumes — a comparison, the clipboard, a file —
            /// where U+2066–U+2069 would be invisible junk.
            ///
            /// # Panics
            ///
            /// As [`to_string`](Self::to_string).
            #[must_use]
            pub fn to_plain_string(&self) -> String {
                ambient(self, true).into_owned()
            }

            /// The text [`to_string`](Self::to_string) returns, borrowed
            /// from the executable when the message is simple and its
            /// catalog is the native store's: no copy, no allocation.
            /// Otherwise it is owned.
            ///
            /// # Panics
            ///
            /// As [`to_string`](Self::to_string).
            #[must_use]
            pub fn to_cow(&self) -> Cow<'static, str> {
                ambient(self, false)
            }

            /// A synonym of [`to_string`](Self::to_string), kept from when
            /// `to_string()` was plain.
            #[cfg(all(
                any(feature = "ssr", feature = "hydrate", feature = "csr"),
                any(feature = "leptos", feature = "leptos-0-8")
            ))]
            #[must_use]
            pub fn to_display_string(&self) -> String {
                ambient(self, false).into_owned()
            }
        }

        impl From<$ty> for String {
            fn from(description: $ty) -> String {
                ambient(&description, false).into_owned()
            }
        }

        /// The text [`to_cow`](Self::to_cow) gives, padded when the format
        /// asks (`{:<12}`).
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                display(self, f)
            }
        }
    };
}

string_conversions!(Tr);
string_conversions!(TrArgs);
string_conversions!(TrRich);
string_conversions!(TrDyn);
