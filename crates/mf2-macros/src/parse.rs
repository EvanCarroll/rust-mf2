//! The input of `__tr_impl!`.
//!
//! ```text
//! __tr_impl!("<abs path>/manifest.mf2m" 0x<hash>u64 ; $crate ; "id", name = value, …)
//! __tr_impl!(bytes b"<manifest>"        0x<hash>u64 ; $crate ; "id", …)
//! ```
//!
//! The application never writes this: the generated module's `tr!` wrapper
//! does, with the path and the hash as literals (D8). The leading
//! `<source> <hash> ;` is split off with the plain `proc_macro` API so that
//! an inline manifest literal is not stringified on a cache hit; the rest is
//! syn's.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitInt, LitStr, Token};

use crate::manifest::Source;

/// One `name = value` at the call site — an argument or a markup handler;
/// which it is, only the manifest says.
pub(crate) struct Arg {
    /// The MF2 name, as written (a Rust identifier, or a string literal for
    /// a name that is not one).
    pub(crate) name: String,
    /// Where to point an error about it.
    pub(crate) span: Span,
    pub(crate) value: Expr,
}

/// The application's part of the call: `"id" (, name = expr)* ,?`.
pub(crate) struct Call {
    pub(crate) id: LitStr,
    pub(crate) args: Vec<Arg>,
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
                // An MF2 name that is not a Rust identifier (05 §3):
                // tr!("id", "odd-name" = value).
                let l: LitStr = input.parse()?;
                (l.value(), l.span())
            } else {
                // `parse_any` so that a keyword-shaped name (`type`, `match`)
                // needs no raw identifier, and `r#type` is accepted as well.
                let i: Ident = input.call(syn::ext::IdentExt::parse_any)?;
                (i.to_string().trim_start_matches("r#").to_owned(), i.span())
            };
            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            args.push(Arg { name, span, value });
        }
        Ok(Call { id, args })
    }
}

/// The whole input.
pub(crate) struct Input {
    pub(crate) source: Source,
    pub(crate) hash: u64,
    /// `$crate` of the i18n crate, through which `__mf2` is reached.
    pub(crate) krate: TokenStream2,
    pub(crate) call: Call,
}

/// `$crate ; "id", …` — everything after the manifest.
struct Tail {
    krate: TokenStream2,
    call: Call,
}

impl Parse for Tail {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut krate = TokenStream2::new();
        loop {
            if input.is_empty() {
                return Err(input.error("expected `;` after the crate path"));
            }
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
                break;
            }
            let tt: TokenTree = input.parse()?;
            krate.extend([tt]);
        }
        let call: Call = input.parse()?;
        Ok(Tail { krate, call })
    }
}

pub(crate) fn input(tokens: TokenStream) -> syn::Result<Input> {
    use proc_macro::TokenTree as T;
    let err = |msg: &str| syn::Error::new(Span::call_site(), msg);
    let mut it = tokens.into_iter();
    let source = match it.next() {
        Some(T::Literal(l)) => {
            Source::Path(syn::parse::<LitStr>(TokenStream::from(T::Literal(l)))?.value())
        }
        Some(T::Ident(i)) if i.to_string() == "bytes" => match it.next() {
            Some(T::Literal(l)) => Source::Bytes(l),
            _ => return Err(err("expected the manifest bytes after `bytes`")),
        },
        _ => return Err(err("expected the manifest path or `bytes`")),
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
    let Tail { krate, call } = syn::parse::<Tail>(it.collect())?;
    Ok(Input {
        source,
        hash,
        krate,
        call,
    })
}
