//! `mf2-macros` — the `tr!` proc-macro.
//!
//! An application never names this crate. `mf2-build` generates, in the i18n
//! crate, an exported `tr!` wrapper that forwards to `__tr_impl` with the
//! manifest's absolute path and hash baked in as literals:
//!
//! ```text
//! __tr_impl!("<abs path>/manifest.mf2m" 0x<hash>u64 ; $crate ; "id", name = value, …)
//! __tr_impl!(bytes b"<manifest>"        0x<hash>u64 ; $crate ; "id", …)
//! ```
//!
//! so that any crate depending on the i18n crate can call `tr!`, with no
//! unstable feature and nothing to configure. The manifest is read once
//! per compiler process, keyed by the path and verified against the baked
//! hash — a manifest that hashes to anything else is reported as stale, never
//! used, which is what keeps a long-lived rust-analyzer proc-macro server
//! honest.
//!
//! `MF2_MACRO_STATS=<file>` makes each rustc process write what the macro
//! cost it — expansions, nanoseconds, manifest reads — which is how the
//! cache and the macro's time are measured (`stats`).
//!
//! What the macro checks and what it emits is the `expand` module's doc; what reaches
//! the wasm is a `MsgId` and the argument values, never an id string, an
//! argument name or a markup name.
//!
//! Both proc-macros are hidden from the documentation: only the generated
//! wrapper calls them, and 1.x promises the `tr!` forms, not these
//! (`docs/versioning.md`).
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! See the [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) for the
//! ecosystem and application guides. An application calls `tr!` through the i18n crate that
//! `mf2-build` generates, and names [`mf2`](https://docs.rs/mf2).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]

mod error;
mod expand;
mod manifest;
mod parse;
mod stats;

use proc_macro::TokenStream;
use quote::quote;

/// The call site, checked against the manifest and lowered to a positional
/// description of the message. Called only by the generated `tr!` wrapper.
#[proc_macro]
#[doc(hidden)]
pub fn __tr_impl(input: TokenStream) -> TokenStream {
    let started = stats::start();
    let out = match parse::input(input).and_then(expand::expand) {
        Ok(tokens) => tokens,
        // Several errors of one call site become several `compile_error!`s;
        // wrapped in a block, so the expansion is still one expression and
        // rustc reports every one of them (a bare sequence misparses in
        // expression position and hides all but the first).
        Err(e) => {
            let errors = e.into_compile_error();
            quote! { { #errors } }
        }
    };
    stats::expansion(started);
    out.into()
}

/// The `MsgId` of a message, checked against the manifest — for a caller
/// that formats with arguments it does not know at compile time
/// (`mf2::TrDyn`). Called only by the generated `msg_id!` wrapper.
#[proc_macro]
#[doc(hidden)]
pub fn __msg_id_impl(input: TokenStream) -> TokenStream {
    let started = stats::start();
    let out = match parse::input(input).and_then(expand::expand_id) {
        Ok(tokens) => tokens,
        Err(e) => {
            let errors = e.into_compile_error();
            quote! { { #errors } }
        }
    };
    stats::expansion(started);
    out.into()
}
