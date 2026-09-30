//! `markup(h)` — the one conversion a rich call site's handler goes through.
//!
//! The macro emits `markup(h)` whatever `h` is, and that expansion is fixed:
//! the core takes anything that already implements [`MarkupHandler`], and
//! the Leptos layer adds the view closure. Which forms the expansion accepts
//! is the Leptos mode's business, not the macro's.
//!
//! | Mode | `markup` takes |
//! |---|---|
//! | none | [`Handler(h)`](Handler) over a [`MarkupHandler`] the caller wrote, or an `Arc<dyn MarkupHandler>` already built |
//! | `ssr`, `hydrate`, `csr` | those, and a **nesting** closure `Fn(AnyView) -> impl IntoAny` or a `Flat` closure `Fn(&MarkupPart) -> impl IntoAny` (`mf2::leptos`) |
//!
//! The Leptos forms erase to one of two concrete handler types, so that the
//! renderer downcasts to a type it knows rather than to the call site's
//! closure. That erasure is why a rich call site costs a thunk rather than a
//! monomorphised renderer.

use alloc::sync::Arc;

use crate::tr::MarkupHandler;

/// `markup(h)` — the one signature, in every build. What `h` may be is what
/// [`IntoMarkupHandler`] is implemented for, and *that* is the mode's
/// business.
#[must_use]
pub fn markup<H: IntoMarkupHandler>(handler: H) -> Arc<dyn MarkupHandler> {
    handler.into_markup_handler()
}

/// `markup_view(h)` — what the macro emits when the handler is written as a
/// closure. In a Leptos mode its bound is the nesting closure's own
/// signature, so `|c| view! { <kbd>{c}</kbd> }` needs no `c: AnyView`: a
/// closure passed where a trait other than `Fn` is expected is never given
/// its argument's type. Elsewhere it is [`markup`].
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
#[must_use]
pub fn markup_view<F, V>(handler: F) -> Arc<dyn MarkupHandler>
where
    F: Fn(crate::line::tachys::view::any_view::AnyView) -> V + Send + Sync + 'static,
    V: crate::line::tachys::view::any_view::IntoAny,
{
    handler.into_markup_handler()
}

/// `markup_view(h)` without a Leptos mode: [`markup`].
#[cfg(not(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
)))]
#[must_use]
pub fn markup_view<H: IntoMarkupHandler>(handler: H) -> Arc<dyn MarkupHandler> {
    handler.into_markup_handler()
}

/// What [`markup`] accepts.
///
/// | Form | Available | What it is |
/// |---|---|---|
/// | [`Handler(h)`](Handler) | always | a [`MarkupHandler`] the caller wrote |
/// | `Arc<dyn MarkupHandler>` | always | one already erased — `markup(markup(h))` is the identity |
/// | `\|children\| view! { … }` | a Leptos mode | a **nesting** handler, called once per `{#name}…{/name}` pair |
/// | `Flat(f)` over `&MarkupPart` | a Leptos mode | a **flat** handler, called once per markup part with no pairing |
///
/// The impls do not overlap, so a rich call site writes the closure and
/// nothing else. A caller's own handler type goes through `Handler` in
/// **every** build rather than bare in some: the same source has to compile
/// whether or not something else in the workspace turned a Leptos mode on.
pub trait IntoMarkupHandler {
    /// The erased handler a description carries.
    fn into_markup_handler(self) -> Arc<dyn MarkupHandler>;
}

/// A [`MarkupHandler`] the caller wrote, on its way through [`markup`].
pub struct Handler<H>(pub H);

/// `Handler(..)`: the handler is the caller's own type, with nothing to
/// show.
impl<H> core::fmt::Debug for Handler<H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Handler(..)")
    }
}

impl<H: MarkupHandler + 'static> IntoMarkupHandler for Handler<H> {
    fn into_markup_handler(self) -> Arc<dyn MarkupHandler> {
        Arc::new(self.0)
    }
}

/// A handler already erased — `markup(markup(h))` is the identity, which is
/// what lets a call site pass one it built itself.
impl IntoMarkupHandler for Arc<dyn MarkupHandler> {
    fn into_markup_handler(self) -> Arc<dyn MarkupHandler> {
        self
    }
}
