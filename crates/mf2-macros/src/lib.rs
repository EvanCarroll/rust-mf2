//! `mf2-macros` — the `tr!` proc-macro (`plans/05-tooling.md` §4,
//! `plans/04-leptos-integration.md` §2).
//!
//! An application never names this crate. `mf2-build` generates, in the i18n
//! crate, an exported `tr!` wrapper that forwards to [`__tr_impl`] with the
//! manifest's absolute path and hash baked in as literals:
//!
//! ```text
//! __tr_impl!("<abs path>/manifest.mf2m" 0x<hash>u64 ; $crate ; "id", name = value, …)
//! __tr_impl!(bytes b"<manifest>"        0x<hash>u64 ; $crate ; "id", …)
//! ```
//!
//! so that any crate depending on the i18n crate can call `tr!`, with no
//! unstable feature and nothing to configure (D8). The manifest is read once
//! per compiler process, keyed by the path and verified against the baked
//! hash — a manifest that hashes to anything else is reported as stale, never
//! used, which is what keeps a long-lived rust-analyzer proc-macro server
//! honest.
//!
//! What the macro checks and what it emits is [`expand`]'s doc; what reaches
//! the wasm is a `MsgId` and the argument values, never an id string, an
//! argument name or a markup name (B6).

#![forbid(unsafe_code)]

mod error;
mod expand;
mod manifest;
mod parse;

use proc_macro::TokenStream;
use quote::quote;

/// The call site, checked against the manifest and lowered to a positional
/// description of the message. Called only by the generated `tr!` wrapper.
#[proc_macro]
pub fn __tr_impl(input: TokenStream) -> TokenStream {
    match parse::input(input).and_then(expand::expand) {
        Ok(tokens) => tokens.into(),
        // Several errors of one call site become several `compile_error!`s;
        // wrapped in a block, so the expansion is still one expression and
        // rustc reports every one of them (a bare sequence misparses in
        // expression position and hides all but the first).
        Err(e) => {
            let errors = e.into_compile_error();
            quote! { { #errors } }.into()
        }
    }
}

/// How many manifests this compiler process has read from disk — the cache's
/// own test (A2: 2,000 expansions read the file once).
///
/// Expands to a `u64` literal, so a test can assert it after expanding many
/// call sites in the same rustc.
#[proc_macro]
pub fn __manifest_reads(_input: TokenStream) -> TokenStream {
    let n = manifest::reads();
    quote! { #n }.into()
}
