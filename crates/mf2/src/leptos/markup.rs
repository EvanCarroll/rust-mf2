//! The markup handlers a Leptos call site writes: the view closure, and the
//! flat handler over the parts (`markup(h)`'s Leptos forms).
//!
//! Both erase to one of two concrete handler types, [`NestingHandler`] and
//! [`FlatHandler`], so that the renderer downcasts to a type it knows rather
//! than to the call site's closure.

use alloc::sync::Arc;
use core::any::Any;

use mf2_runtime::MarkupPart;

use crate::line::tachys;
use crate::markup::IntoMarkupHandler;
use crate::tr::MarkupHandler;
use tachys::view::any_view::{AnyView, IntoAny};

/// A **nesting** handler: called with the span's inner fragment as its
/// children, once per `{#name}…{/name}` pair.
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

/// `NestingHandler(..)`: the call site's closure, with nothing to show.
impl core::fmt::Debug for NestingHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NestingHandler(..)")
    }
}

/// A **flat** handler: called once per markup part, with no pairing at all —
/// what conformance L6 compares against `expParts`, and what an application
/// uses when it wants the parts as the catalog gives them.
pub struct Flat<F>(pub F);

/// `Flat(..)`: the call site's closure, with nothing to show.
impl<F> core::fmt::Debug for Flat<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Flat(..)")
    }
}

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

/// `FlatHandler(..)`: the call site's closure, with nothing to show.
impl core::fmt::Debug for FlatHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FlatHandler(..)")
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
