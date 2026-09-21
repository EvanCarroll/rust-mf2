//! P0.9 stand-in for the `mf2` facade (plans/05-tooling.md §9): the types a
//! `tr!` expansion names through `$crate::__mf2` — `MsgId`, `Tr`, `TrArgs`,
//! `ArgValue`, `tr`, `tr_args` — plus server-side catalogs and just enough
//! tachys glue for the reference-workload app to build and render under SSR
//! and hydrate. Rendering fidelity is P0.1/P0.2's subject, not this probe's:
//! the client has no catalog here (hydration adopts the server's text).

#![forbid(unsafe_code)]
#![allow(clippy::inherent_to_string)]

pub mod error;
mod format;
mod glue;

use std::borrow::Cow;

use leptos::prelude::*;

pub use error::InstallError;
pub use format::{catalog_bytes, current_locale, install};

/// A message id: dense index in bytewise-sorted id order (plans/02 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MsgId(pub u32);

/// A message without arguments: `Copy`, 4 bytes, `const`-constructible.
#[derive(Clone, Copy, Debug)]
pub struct Tr {
    id: MsgId,
}

const _: () = assert!(size_of::<Tr>() == 4);

/// `tr!` expansion for a message without variables.
#[inline(always)]
pub const fn tr(id: MsgId) -> Tr {
    Tr { id }
}

impl Tr {
    /// The id.
    pub const fn id(&self) -> MsgId {
        self.id
    }
    /// Formats in the current locale.
    pub fn to_string(&self) -> String {
        format::format(self.id, &[])
    }
}

/// An owned call-site argument value (plans/04 §2 `ArgValue`, reduced).
#[derive(Clone, Debug)]
pub enum ArgValue {
    /// Integer.
    Int(i64),
    /// Text.
    Str(Cow<'static, str>),
    /// Signal-valued integer.
    SigInt(Signal<i64>),
    /// Signal-valued text.
    SigStr(Signal<String>),
}

impl From<i64> for ArgValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<&'static str> for ArgValue {
    fn from(v: &'static str) -> Self {
        Self::Str(Cow::Borrowed(v))
    }
}
impl From<String> for ArgValue {
    fn from(v: String) -> Self {
        Self::Str(Cow::Owned(v))
    }
}
impl From<ReadSignal<i64>> for ArgValue {
    fn from(v: ReadSignal<i64>) -> Self {
        Self::SigInt(v.into())
    }
}
impl From<ReadSignal<String>> for ArgValue {
    fn from(v: ReadSignal<String>) -> Self {
        Self::SigStr(v.into())
    }
}
impl From<Signal<i64>> for ArgValue {
    fn from(v: Signal<i64>) -> Self {
        Self::SigInt(v)
    }
}
impl From<Signal<String>> for ArgValue {
    fn from(v: Signal<String>) -> Self {
        Self::SigStr(v)
    }
}

impl ArgValue {
    fn text(&self) -> Cow<'_, str> {
        match self {
            Self::Int(v) => Cow::Owned(v.to_string()),
            Self::Str(s) => Cow::Borrowed(s),
            Self::SigInt(s) => Cow::Owned(s.get_untracked().to_string()),
            Self::SigStr(s) => Cow::Owned(s.get_untracked()),
        }
    }
    fn int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Str(s) => s.parse().ok(),
            Self::SigInt(s) => Some(s.get_untracked()),
            Self::SigStr(s) => s.get_untracked().parse().ok(),
        }
    }
}

/// A message with arguments, in slot order.
#[derive(Clone, Debug)]
pub struct TrArgs {
    id: MsgId,
    args: Vec<ArgValue>,
}

/// `tr!` expansion for a message with variables (values in slot order).
pub fn tr_args<const N: usize>(id: MsgId, args: [ArgValue; N]) -> TrArgs {
    TrArgs {
        id,
        args: args.into(),
    }
}

impl TrArgs {
    /// The id.
    pub const fn id(&self) -> MsgId {
        self.id
    }
    /// Formats in the current locale.
    pub fn to_string(&self) -> String {
        format::format(self.id, &self.args)
    }
}

/// One row of the generated locale table.
#[derive(Clone, Copy, Debug)]
pub struct LocaleInfo {
    /// BCP 47 tag.
    pub tag: &'static str,
    /// Right-to-left.
    pub rtl: bool,
}

/// One embedded (server-only) catalog.
#[derive(Clone, Copy, Debug)]
pub struct CatalogFile {
    /// Locale tag.
    pub tag: &'static str,
    /// Content-hashed file name.
    pub file: &'static str,
    /// Content hash.
    pub hash: u64,
    /// The bytes.
    pub bytes: &'static [u8],
}

/// Client boot check (P0.9): compares the manifest hash compiled into this
/// wasm with the one the server rendered into `<meta name="mf2-manifest">`,
/// and logs the verdict to the console.
#[cfg(feature = "hydrate")]
pub fn client_check(compiled: u64) {
    let server = document()
        .query_selector("meta[name=mf2-manifest]")
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"));
    let mine = std::format!("{compiled:016x}");
    match server {
        Some(s) if s == mine => leptos::logging::log!("mf2: manifest ok {mine}"),
        Some(s) => leptos::logging::warn!("mf2: MANIFEST SKEW server={s} client={mine}"),
        None => leptos::logging::warn!("mf2: no manifest meta; client={mine}"),
    }
}
