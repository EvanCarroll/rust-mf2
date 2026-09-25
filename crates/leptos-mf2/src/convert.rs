//! The conversions a component prop asks for
//! (`plans/04-leptos-integration.md` §3, §5).
//!
//! `#[prop(into)] label: TextProp` — and `Signal<String>`, `Oco<'static,
//! str>`, `String` — are how a Leptos component takes text, so a description
//! converts into each of them. Every one is **one function in the library**,
//! not one per call site (B5).
//!
//! Two of them are *derived*: they hand back a closure the consumer calls
//! later, in its own reactive context. That is where D9 bites.
//!
//! * On the **client** the closure subscribes to the locale-change trigger,
//!   so a component that holds a `TextProp` re-reads after a switch.
//! * On the **server** the closure captures this request's catalog at
//!   conversion time, because a third party may evaluate it outside the
//!   request owner: `leptos_meta` reads `<Title text>` *after* rendering, and
//!   P0.2 measured the `<title>` coming out in the default locale under
//!   `PartiallyBlocked` and `Async` without this. `Tr` itself stays a
//!   four-byte `Copy` value — only the derived conversions capture.

use alloc::string::String;
use alloc::sync::Arc;

use leptos::text_prop::TextProp;
use oco_ref::Oco;
use reactive_graph::wrappers::read::Signal;

use crate::state::TextUse;
use crate::text::{self, Description};
use crate::{Tr, TrArgs, TrDyn, TrRich};

/// A closure that produces the message's text whenever it is called.
///
/// Text handed to a component prop is text a person will read, so it is
/// isolated (§9).
fn reader<D: Description>(description: D) -> impl Fn() -> String + Send + Sync + 'static {
    #[cfg(feature = "ssr")]
    {
        // D9, as P0.2 narrowed it: capture inside the request owner — the
        // catalog, and the reader's time zone with it.
        let request = crate::catalog::current();
        move || match &request {
            Some(request) => request.in_zone(|| {
                text::with_text(&description, request.catalog(), TextUse::Displayed, |t| {
                    String::from(t)
                })
            }),
            None => String::new(),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        move || {
            // Subscribing here is what makes the consumer's own effect re-run
            // after `set_locale`; the node registry handles the nodes we
            // rendered ourselves. The subscription ends with the consumer's
            // run, so a consumer in a churning row leaves nothing behind.
            crate::catalog::track_locale();
            text::to_string(&description, TextUse::Displayed)
        }
    }
}

/// `TextProp`, `Signal<String>` and `Oco` for one description type.
macro_rules! prop_conversions {
    ($ty:ty) => {
        /// Derived: re-read after a locale switch (client), captured for the
        /// request (server).
        impl From<$ty> for TextProp {
            fn from(description: $ty) -> TextProp {
                TextProp::from(reader(description))
            }
        }

        /// Derived, like [`TextProp`].
        impl From<$ty> for Signal<String> {
            fn from(description: $ty) -> Signal<String> {
                Signal::derive(reader(description))
            }
        }

        /// **Not** derived: an `Oco` is a value, so this is the message's
        /// text now. It is the cheapest prop — and the one a component holds
        /// across a locale switch without noticing, which is why
        /// `#[prop(into)] TextProp` is what the documentation steers to.
        impl From<$ty> for Oco<'static, str> {
            fn from(description: $ty) -> Oco<'static, str> {
                Oco::Counted(Arc::from(text::to_string(&description, TextUse::Displayed)))
            }
        }
    };
}

prop_conversions!(Tr);
prop_conversions!(TrArgs);
prop_conversions!(TrRich);
prop_conversions!(TrDyn);
