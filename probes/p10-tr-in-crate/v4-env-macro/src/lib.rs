//! Two stand-ins for an env-driven `tr!`. Both forward to the real
//! `mf2::__tr_impl!` (which checks the call against the manifest and emits
//! the description), so only *how the manifest is found* differs:
//!
//! * `tr_env!` — the path in `MF2_MANIFEST` and the hash in
//!   `MF2_MANIFEST_HASH`, both printed by the build script as
//!   `cargo::rustc-env`;
//! * `tr_outdir!` — `$OUT_DIR/manifest.mf2m`, `OUT_DIR` being set by cargo
//!   for every compilation of a package with a build script; the hash read
//!   from the file itself.
//!
//! Each expansion appends one line to `$MF2_PROBE_TRACE`, when set, naming
//! the path it used — which is how the relocation scenario is observed.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

fn trace(line: &str) {
    if let Some(file) = std::env::var_os("MF2_PROBE_TRACE") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn error(msg: &str) -> TokenStream {
    quote! { ::core::compile_error!(#msg) }.into()
}

fn forward(path: &str, hash: u64, input: TokenStream) -> TokenStream {
    let rest: TokenStream2 = input.into();
    let lit = proc_macro2::Literal::string(path);
    let hash = proc_macro2::Literal::u64_suffixed(hash);
    quote! { ::mf2::__tr_impl!(#lit #hash ; crate ; #rest) }.into()
}

/// `tr_env!("id", …)`: the manifest named by the build script's
/// `cargo::rustc-env=MF2_MANIFEST=…` / `MF2_MANIFEST_HASH=…`.
#[proc_macro]
pub fn tr_env(input: TokenStream) -> TokenStream {
    let Ok(path) = std::env::var("MF2_MANIFEST") else {
        return error("tr_env!: MF2_MANIFEST is not set — this crate's build script must run mf2-build");
    };
    let Some(hash) = std::env::var("MF2_MANIFEST_HASH")
        .ok()
        .and_then(|h| h.parse::<u64>().ok())
    else {
        return error("tr_env!: MF2_MANIFEST_HASH is not set");
    };
    trace(&format!("tr_env MF2_MANIFEST={path} OUT_DIR={}", std::env::var("OUT_DIR").unwrap_or_default()));
    forward(&path, hash, input)
}

/// `tr_outdir!("id", …)`: `$OUT_DIR/manifest.mf2m`, whatever the build
/// script printed.
#[proc_macro]
pub fn tr_outdir(input: TokenStream) -> TokenStream {
    let Ok(out) = std::env::var("OUT_DIR") else {
        return error("tr_outdir!: OUT_DIR is not set — this crate has no build script");
    };
    let path = std::path::Path::new(&out).join("manifest.mf2m");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => return error(&format!("tr_outdir!: {}: {e}", path.display())),
    };
    let hash = match mf2_catalog::Manifest::read(&bytes) {
        Ok(m) => m.hash(),
        Err(e) => return error(&format!("tr_outdir!: {}: {e}", path.display())),
    };
    let path = path.display().to_string();
    trace(&format!("tr_outdir {path}"));
    forward(&path, hash, input)
}
