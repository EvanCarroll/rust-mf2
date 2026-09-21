//! P0.9 stand-in for `mf2-macros` (plans/05-tooling.md §4, plans/04 §2).
//!
//! Never called directly: the i18n crate's generated `tr!` wrapper forwards
//! `tr!(…)` as
//!
//! ```text
//! __tr_impl!("/abs/OUT_DIR/manifest.mf2m" 0x<manifest_hash>u64 ; $crate ; "id", a = x, …)
//! __tr_impl!(bytes b"<manifest bytes>" 0x<manifest_hash>u64 ; $crate ; …)   // inline fallback
//! ```
//!
//! The manifest is read once per compiler process (cached by path and
//! verified against the baked hash, so a long-lived rust-analyzer proc-macro
//! server never serves a stale one). Checks: the id exists (did-you-mean),
//! the argument names equal the message's variables (no unknown, missing or
//! duplicate names). Emits `$crate::__mf2::tr(MsgId(n))` or
//! `$crate::__mf2::tr_args(MsgId(n), [ArgValue::from(..), ..])` in slot order.

#![forbid(unsafe_code)]

mod error;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use p09_catalog::Manifest;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitByteStr, LitInt, LitStr, Token};

use crate::error::ManifestError;

/// Where the manifest comes from.
enum Source {
    Path(LitStr),
    /// Kept as the raw compiler literal: it is only turned into a string (and
    /// unescaped) on a cache miss, i.e. once per compiler process.
    Bytes(proc_macro::Literal),
}

/// One `name = value` argument.
struct Arg {
    name: String,
    span: Span,
    value: Expr,
}

/// The user's part: `"id" (, name = expr)* ,?`.
struct Call {
    id: LitStr,
    args: Vec<Arg>,
}

impl Parse for Call {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if !input.peek(LitStr) {
            return Err(input
                .error("tr! expects a string-literal message id: tr!(\"id\", name = value, …)"));
        }
        let id: LitStr = input.parse()?;
        let mut args = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let (name, span) = if input.peek(LitStr) {
                // MF2 names that are not Rust identifiers: tr!("id", "odd-name" = v)
                let l: LitStr = input.parse()?;
                (l.value(), l.span())
            } else {
                let i: Ident = input.call(syn::ext::IdentExt::parse_any)?;
                (i.to_string().trim_start_matches("r#").to_owned(), i.span())
            };
            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            args.push(Arg { name, span, value });
        }
        Ok(Self { id, args })
    }
}

/// Tokens up to (not including) the next top-level `;`, which is consumed.
fn until_semi(input: ParseStream<'_>) -> syn::Result<TokenStream2> {
    let mut out = TokenStream2::new();
    loop {
        if input.is_empty() {
            return Err(input.error("expected `;`"));
        }
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
            return Ok(out);
        }
        let tt: TokenTree = input.parse()?;
        out.extend([tt]);
    }
}

/// The whole input of `__tr_impl!`.
struct Input {
    source: Source,
    hash: u64,
    krate: TokenStream2,
    call: Call,
}

/// Splits `<source> <hash> ;` off with the plain `proc_macro` API (no syn,
/// so the inline manifest literal is never stringified on a cache hit), then
/// parses the rest (`$crate ; "id", …`) with syn.
fn parse_input(input: TokenStream) -> syn::Result<Input> {
    use proc_macro::TokenTree as T;
    let err = |msg: &str| syn::Error::new(Span::call_site(), msg);
    let mut it = input.into_iter();
    let source = match it.next() {
        Some(T::Literal(l)) => {
            Source::Path(syn::parse::<LitStr>(TokenStream::from(T::Literal(l)))?)
        }
        Some(T::Ident(i)) if i.to_string() == "bytes" => match it.next() {
            Some(T::Literal(l)) => Source::Bytes(l),
            _ => return Err(err("expected the manifest bytes after `bytes`")),
        },
        _ => return Err(err("expected a manifest path or `bytes`")),
    };
    let hash = match it.next() {
        Some(T::Literal(l)) => {
            syn::parse::<LitInt>(TokenStream::from(T::Literal(l)))?.base10_parse::<u64>()?
        }
        _ => return Err(err("expected the manifest hash")),
    };
    match it.next() {
        Some(T::Punct(p)) if p.as_char() == ';' => {}
        _ => return Err(err("expected `;` after the manifest hash")),
    }
    let Input0 { krate, call } = syn::parse::<Input0>(it.collect())?;
    Ok(Input {
        source,
        hash,
        krate,
        call,
    })
}

/// The trivial control's input: `$crate ; "id", …`.
struct Input0 {
    krate: TokenStream2,
    call: Call,
}

impl Parse for Input0 {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let krate = until_semi(input)?;
        let call: Call = input.parse()?;
        Ok(Self { krate, call })
    }
}

type Cache = Mutex<HashMap<String, Arc<Manifest>>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Loads (or reuses) the manifest the wrapper was generated for.
fn manifest(source: &Source, hash: u64) -> Result<Arc<Manifest>, ManifestError> {
    let key = match source {
        Source::Path(p) => p.value(),
        Source::Bytes(_) => format!("inline:{hash:016x}"),
    };
    let mut map = cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(m) = map.get(&key)
        && m.hash == hash
    {
        return Ok(Arc::clone(m));
    }
    stats(|s| s.reads += 1);
    let bytes = match source {
        Source::Path(p) => {
            let path = p.value();
            match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => match relocated(&path) {
                    Some(alt) => {
                        std::fs::read(&alt).map_err(|e| ManifestError::Read { path, source: e })?
                    }
                    None => return Err(ManifestError::Read { path, source: e }),
                },
            }
        }
        Source::Bytes(l) => {
            syn::parse::<LitByteStr>(TokenStream::from(proc_macro::TokenTree::Literal(l.clone())))
                .map_err(|e| ManifestError::Literal(e.to_string()))?
                .value()
        }
    };
    let m = Manifest::decode(&bytes)?;
    if m.hash != hash {
        return Err(ManifestError::Stale {
            path: key,
            found: m.hash,
            expected: hash,
        });
    }
    let m = Arc::new(m);
    map.insert(key, Arc::clone(&m));
    Ok(m)
}

/// Fallback for a moved target directory (e.g. a CI cache restored at another
/// path): the i18n crate is fresh, so its build script did not rerun and the
/// baked OUT_DIR path is gone. Look for the same
/// `build/<pkg>-<hash>/out/manifest.mf2m` under the profile directories of the
/// current compilation, which rustc (our host process) receives as
/// `-L dependency=<target>[/<triple>]/<profile>/deps`. The caller still verifies
/// the manifest hash, so a wrong file cannot be used.
fn relocated(baked: &str) -> Option<std::path::PathBuf> {
    use std::path::{Component, Path, PathBuf};
    let comps: Vec<Component<'_>> = Path::new(baked).components().collect();
    let i = comps.iter().rposition(|c| c.as_os_str() == "build")?;
    let suffix: PathBuf = comps[i..].iter().collect();
    let mut args = std::env::args_os();
    while let Some(a) = args.next() {
        let v = if a == "-L" { args.next()? } else { a };
        let v = v.to_string_lossy();
        let v = v.strip_prefix("-L").unwrap_or(&v);
        if let Some(deps) = v.strip_prefix("dependency=")
            && let Some(profile) = Path::new(deps).parent()
        {
            let candidate = profile.join(&suffix);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[derive(Default)]
struct Stats {
    expansions: u64,
    nanos: u128,
    reads: u64,
}

/// Self-timing, only when `P09_MACRO_STATS=<file>` is set (P0.9 measurement).
fn stats(f: impl FnOnce(&mut Stats)) {
    static STATS: OnceLock<Option<Mutex<Stats>>> = OnceLock::new();
    let s = STATS
        .get_or_init(|| std::env::var_os("P09_MACRO_STATS").map(|_| Mutex::new(Stats::default())));
    if let Some(m) = s {
        let mut g = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut g);
    }
}

fn flush_stats() {
    let Some(path) = std::env::var_os("P09_MACRO_STATS") else {
        return;
    };
    stats(|s| {
        let line = format!(
            "pid {} expansions {} nanos {} reads {}\n",
            std::process::id(),
            s.expansions,
            s.nanos,
            s.reads
        );
        let mut p = std::path::PathBuf::from(&path);
        let name = format!(
            "{}.{}",
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            std::process::id()
        );
        p.set_file_name(name);
        let _ = std::fs::write(p, line);
    });
}

/// Levenshtein distance, for did-you-mean.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn suggest<'a>(wanted: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (wanted.chars().count() / 3).max(2);
    candidates
        .map(|c| (distance(wanted, c), c))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, c)| c)
}

fn list(vars: &[String]) -> String {
    if vars.is_empty() {
        return "none".into();
    }
    vars.iter()
        .map(|v| format!("${v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn expand(input: Input) -> syn::Result<TokenStream2> {
    let Input {
        source,
        hash,
        krate,
        call,
    } = input;
    let m = manifest(&source, hash).map_err(|e| syn::Error::new(call.id.span(), e.to_string()))?;
    let id = call.id.value();
    let Some((msg_id, entry)) = m.lookup(&id) else {
        let hint = suggest(&id, m.entries.iter().map(|e| e.id.as_str()))
            .map(|s| format!("; did you mean `{s}`?"))
            .unwrap_or_default();
        return Err(syn::Error::new(
            call.id.span(),
            format!("unknown message id `{id}`{hint}"),
        ));
    };
    let mut errors: Option<syn::Error> = None;
    let mut push = |e: syn::Error| match &mut errors {
        Some(all) => all.combine(e),
        None => errors = Some(e),
    };
    let mut slots: Vec<Option<&Expr>> = vec![None; entry.vars.len()];
    for a in &call.args {
        match entry.vars.iter().position(|v| *v == a.name) {
            Some(i) if slots[i].is_some() => push(syn::Error::new(
                a.span,
                format!("argument `{}` given twice", a.name),
            )),
            Some(i) => slots[i] = Some(&a.value),
            None => {
                let hint = suggest(&a.name, entry.vars.iter().map(String::as_str))
                    .map(|s| format!("; did you mean `{s}`?"))
                    .unwrap_or_default();
                push(syn::Error::new(
                    a.span,
                    format!(
                        "message `{id}` has no variable `${}` (its variables: {}){hint}",
                        a.name,
                        list(&entry.vars)
                    ),
                ));
            }
        }
    }
    let missing: Vec<String> = entry
        .vars
        .iter()
        .zip(&slots)
        .filter(|(_, s)| s.is_none())
        .map(|(v, _)| format!("`{v}`"))
        .collect();
    if !missing.is_empty() {
        push(syn::Error::new(
            call.id.span(),
            format!(
                "message `{id}` needs argument{} {} (its variables: {}); write tr!(\"{id}\", {})",
                if missing.len() == 1 { "" } else { "s" },
                missing.join(", "),
                list(&entry.vars),
                entry
                    .vars
                    .iter()
                    .map(|v| format!("{v} = …"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if let Some(e) = errors {
        return Err(e);
    }
    // The emitted tokens carry the span of the user's id literal, so debug
    // info and type errors in the expansion point at the call site rather than
    // into the generated wrapper in the i18n crate's OUT_DIR. (Build time is the
    // same either way: scripts/time-spans.sh, P0.9 RESULT.md.)
    let span = call.id.span();
    let mut n = proc_macro2::Literal::u32_unsuffixed(msg_id);
    n.set_span(span);
    if slots.is_empty() {
        return Ok(quote_spanned! {span=> #krate::__mf2::tr(#krate::__mf2::MsgId(#n)) });
    }
    let values = slots.into_iter().flatten();
    Ok(quote_spanned! {span=>
        #krate::__mf2::tr_args(#krate::__mf2::MsgId(#n), [#(#krate::__mf2::ArgValue::from(#values)),*])
    })
}

/// The real macro (called by the generated `tr!` wrapper).
#[proc_macro]
pub fn tr_impl(input: TokenStream) -> TokenStream {
    let t0 = Instant::now();
    let out = match parse_input(input).and_then(expand) {
        Ok(ts) => ts,
        // Several errors become several `compile_error!`s: wrap them in a block so
        // the expansion is still one expression and every error is reported.
        Err(e) => {
            let errors = e.into_compile_error();
            quote! { { #errors } }
        }
    };
    let nanos = t0.elapsed().as_nanos();
    stats(|s| {
        s.expansions += 1;
        s.nanos += nanos;
    });
    flush_stats();
    out.into()
}

/// Timing control: parses the same call syntax, reads no manifest, checks
/// nothing, emits the same shape with `MsgId(0)`.
#[proc_macro]
pub fn tr0_impl(input: TokenStream) -> TokenStream {
    let Input0 { krate, call } = match syn::parse::<Input0>(input) {
        Ok(i) => i,
        Err(e) => return e.into_compile_error().into(),
    };
    if call.args.is_empty() {
        return quote! { #krate::__mf2::tr(#krate::__mf2::MsgId(0)) }.into();
    }
    let values = call.args.iter().map(|a| &a.value);
    quote! {
        #krate::__mf2::tr_args(#krate::__mf2::MsgId(0), [#(#krate::__mf2::ArgValue::from(#values)),*])
    }
    .into()
}
