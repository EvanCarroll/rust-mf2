//! A description's text as a `String`, and `Display` — the ambient forms,
//! which read the catalog in force: the request's under `ssr`, the page's
//! under `hydrate` and `csr`.
//!
//! One text path per description type, shared by three ways to it:
//!
//! * the inherent `to_string()`, `to_plain_string()` and `String::from` —
//!   **fmt-free**, and what browser code calls: an inherent method wins name
//!   resolution over `ToString::to_string`, so `tr.to_string()` never
//!   reaches `Display`;
//! * `Display` — `format!`, `println!`, `write!`, and anything generic over
//!   `Display` or `ToString`. It pads the text `to_string()` builds, so
//!   `{:<12}` pads, and costs one more `String` than `to_string()`. In a
//!   browser build `{}` adds a few dozen bytes over `.to_string()` in an
//!   application that already formats with `format!`; a client that never
//!   writes one links none of it.
//!
//! A build with no mode has no ambient text: nothing says which catalog to
//! read, so there is neither `to_string()` nor `Display` there.

use alloc::string::String;
use core::fmt;

use crate::leptos::state::TextUse;
use crate::leptos::text::{self, Description};
use crate::{Tr, TrArgs, TrDyn, TrRich};

/// The text of `description` in the catalog in force: the one text path the
/// conversions below share.
fn ambient<D: Description>(description: &D, use_: TextUse) -> String {
    text::to_string(description, use_)
}

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

string_conversions!(Tr);
string_conversions!(TrArgs);
string_conversions!(TrRich);
string_conversions!(TrDyn);
