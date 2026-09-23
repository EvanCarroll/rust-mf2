//! `markup(h)` — the one conversion a rich call site's handler goes through
//! (`plans/04-leptos-integration.md` §2.1, §7).
//!
//! The macro emits `markup(h)` whatever `h` is, and that expansion is fixed:
//! Phase 5b's core took anything that already implemented [`MarkupHandler`],
//! and Phase 6 adds the view closure. Which `markup` the expansion reaches is
//! the `leptos` feature's business, not the macro's.
//!
//! | Feature | `markup` takes |
//! |---|---|
//! | off | `H: MarkupHandler + 'static` — the core form: a handler type the caller wrote |
//! | on | [`IntoMarkupHandler`]: a **nesting** closure `Fn(AnyView) -> impl IntoAny`, a [`Flat`] closure `Fn(&MarkupPart) -> impl IntoAny`, or an `Arc<dyn MarkupHandler>` already built |
//!
//! Both erase to one of two concrete handler types, so that the renderer
//! downcasts to a type it knows rather than to the call site's closure:
//! [`NestingHandler`] and [`FlatHandler`]. That erasure is why a rich call
//! site costs a thunk rather than a monomorphised renderer (§7).

use alloc::sync::Arc;

use crate::tr::MarkupHandler;

/// `markup(h)` — the one signature, in both builds. What `h` may be is what
/// [`IntoMarkupHandler`] is implemented for, and *that* is the feature's
/// business.
#[must_use]
pub fn markup<H: IntoMarkupHandler>(handler: H) -> Arc<dyn MarkupHandler> {
    handler.into_markup_handler()
}

/// What [`markup`] accepts.
///
/// | Form | Available | What it is |
/// |---|---|---|
/// | [`Handler(h)`](Handler) | always | a [`MarkupHandler`] the caller wrote |
/// | `Arc<dyn MarkupHandler>` | always | one already erased — `markup(markup(h))` is the identity |
/// | `\|children\| view! { … }` | `leptos` | a **nesting** handler, called once per `{#name}…{/name}` pair |
/// | [`Flat(f)`](Flat) over `&MarkupPart` | `leptos` | a **flat** handler, called once per markup part with no pairing |
///
/// The impls do not overlap, so a rich call site writes the closure and
/// nothing else. A caller's own handler type goes through `Handler` in
/// **both** builds rather than bare in one of them: the same source has to
/// compile whether or not something else in the workspace turned `leptos`
/// on.
pub trait IntoMarkupHandler {
    /// The erased handler a description carries.
    fn into_markup_handler(self) -> Arc<dyn MarkupHandler>;
}

/// A [`MarkupHandler`] the caller wrote, on its way through [`markup`].
pub struct Handler<H>(pub H);

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

#[cfg(feature = "leptos")]
pub use self::leptos_markup::{Flat, FlatHandler, NestingHandler};

#[cfg(feature = "leptos")]
mod leptos_markup {
    use alloc::sync::Arc;
    use core::any::Any;

    use mf2_runtime::MarkupPart;
    use tachys::view::any_view::{AnyView, IntoAny};

    use super::IntoMarkupHandler;
    use crate::tr::MarkupHandler;

    /// A **nesting** handler: called with the span's inner fragment as its
    /// children, once per `{#name}…{/name}` pair (`plans/04` §7).
    pub struct NestingHandler(Arc<dyn Fn(AnyView) -> AnyView + Send + Sync>);

    impl NestingHandler {
        /// Wraps `children` in whatever the call site's closure builds.
        #[must_use]
        pub fn call(&self, children: AnyView) -> AnyView {
            (self.0)(children)
        }
    }

    impl MarkupHandler for NestingHandler {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    /// A **flat** handler: called once per markup part, with no pairing at
    /// all — what conformance L6 compares against `expParts`, and what an
    /// application uses when it wants the parts as the catalog gives them.
    pub struct Flat<F>(pub F);

    /// [`Flat`], erased.
    pub struct FlatHandler(Arc<dyn Fn(&MarkupPart<'_>) -> AnyView + Send + Sync>);

    impl FlatHandler {
        /// Renders one markup part.
        #[must_use]
        pub fn call(&self, part: &MarkupPart<'_>) -> AnyView {
            (self.0)(part)
        }
    }

    impl MarkupHandler for FlatHandler {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    /// The common case: the closure a call site writes inside `tr!`.
    impl<F, V> IntoMarkupHandler for F
    where
        F: Fn(AnyView) -> V + Send + Sync + 'static,
        V: IntoAny,
    {
        fn into_markup_handler(self) -> Arc<dyn MarkupHandler> {
            Arc::new(NestingHandler(Arc::new(move |children| {
                self(children).into_any()
            })))
        }
    }

    impl<F, V> IntoMarkupHandler for Flat<F>
    where
        F: for<'p> Fn(&MarkupPart<'p>) -> V + Send + Sync + 'static,
        V: IntoAny,
    {
        fn into_markup_handler(self) -> Arc<dyn MarkupHandler> {
            let f = self.0;
            Arc::new(FlatHandler(Arc::new(move |part| f(part).into_any())))
        }
    }
}
