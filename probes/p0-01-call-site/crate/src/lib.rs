//! P0.1 / P0.11 probe: the concrete call-site types of plans/04 §2–4.
//!
//! * [`Tr`] — a message without arguments: `MsgId`, `Copy`, 4 bytes, built by
//!   the `const fn` [`tr`].
//! * [`TrArgs`] — **one** concrete type for every message with arguments:
//!   `MsgId` + an [`ArgList`] holding up to four [`ArgValue`]s inline and
//!   spilling to a boxed slice beyond that.
//!
//! Rendering as a text child and as an attribute, `From` into `TextProp` /
//! `Signal<String>` / `Oco<'static, str>` / `String`, and `to_string()` are
//! implemented **once**, here; a call site only constructs the description.
//!
//! The catalog is a stub (text by index, arguments appended), filled at run
//! time so the optimiser cannot fold a site's id away. The node update
//! strategy is B (library-owned registry) by default, A (one `RenderEffect`
//! per node) with feature `effect`.
//!
//! Client-path discipline: no `unsafe`, no `format!` / `Debug` / `Display`,
//! no `unwrap`, no panicking indexing (lints in `Cargo.toml`).

mod args;
mod catalog;
mod convert;
mod format;
mod glue;

#[cfg(not(any(feature = "effect", feature = "static-locale")))]
#[path = "update/registry.rs"]
mod update;

#[cfg(feature = "effect")]
#[path = "update/effect.rs"]
mod update;

#[cfg(all(feature = "static-locale", not(feature = "effect")))]
#[path = "update/none.rs"]
mod update;

pub use args::{ArgList, ArgValue};
pub use catalog::{ServerCatalog, boot_from_dom, set_catalog};
pub use update::{Binding, TrText};

/// Dense message id assigned by the build (plans/05 §3).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MsgId(pub u32);

/// A message without variables: the whole call site is this 4-byte value.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Tr {
    id: MsgId,
}

/// A message with variables: id + positional arguments.
#[derive(Clone)]
pub struct TrArgs {
    id: MsgId,
    args: ArgList,
}

/// `tr!("id")` expands to this.
#[inline(always)]
pub const fn tr(id: u32) -> Tr {
    Tr { id: MsgId(id) }
}

/// `tr!("id", a = …, b = …)` expands to this, arguments in slot order.
///
/// Generic over the array length only (one instance per arity, 1–4 in the
/// reference workload); the result is the one concrete type [`TrArgs`].
#[inline(never)]
pub fn tr_args<const N: usize>(id: u32, args: [ArgValue; N]) -> TrArgs {
    TrArgs {
        id: MsgId(id),
        args: ArgList::from_array(args),
    }
}

impl Tr {
    /// The message id.
    pub const fn id(self) -> MsgId {
        self.id
    }

    /// Formats the message with the active catalog. Tracks the locale when
    /// called inside a reactive observer; never warns outside one.
    #[inline(never)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(self) -> String {
        catalog::track_locale();
        format::resolve_string(self.id, &[], format::Tracking::Tracked)
    }
}

impl TrArgs {
    /// The message id.
    pub const fn id(&self) -> MsgId {
        self.id
    }

    /// The positional arguments.
    pub fn args(&self) -> &[ArgValue] {
        self.args.as_slice()
    }

    /// Formats the message with the active catalog (tracks the locale and any
    /// reactive argument when called inside an observer).
    #[inline(never)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        catalog::track_locale();
        format::resolve_string(self.id, self.args.as_slice(), format::Tracking::Tracked)
    }

    pub(crate) fn into_parts(self) -> (MsgId, Box<[ArgValue]>) {
        (self.id, self.args.into_boxed())
    }
}

/// Number of live registry slots (strategy B) or live bindings (strategy A,
/// counted the same way), for the P0.11 harness.
pub fn live_bindings() -> usize {
    update::live()
}
