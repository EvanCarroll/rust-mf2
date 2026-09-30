//! What a call site is checked against, and what it becomes
//! (`plans/04-leptos-integration.md` §2, `plans/05-tooling.md` §4).
//!
//! Checked against the manifest: the id exists, the argument names are
//! exactly the message's variables, and — for a call site that supplies
//! markup handlers at all — every markup name of the message has one.
//! Emitted: a positional construction, spanned at the id literal, with no id
//! string, no argument name and no markup name in it. Each argument is
//! converted by `mf2::__arg`'s dispatch (`plans/19-native-and-terminal.md`
//! §7), spanned at the argument, so a type that is not an argument is
//! reported there.

use std::sync::Arc;

use mf2_catalog::{Manifest, markup_key};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote_spanned;
use syn::Expr;

use crate::parse::{Call, Input};

/// `msg_id!("id")`: the id checked against the manifest, and nothing else —
/// the `MsgId` a caller needs to format a message whose arguments are not
/// known until run time (`mf2::TrDyn`, `plans/13-phase-5b-work-order.md` A3).
/// It takes no arguments, so there is no argument set to check.
pub(crate) fn expand_id(input: Input) -> syn::Result<TokenStream> {
    let Input {
        source,
        hash,
        krate,
        call,
    } = input;
    let manifest = crate::manifest::load(&source, hash)
        .map_err(|e| syn::Error::new(call.id.span(), e.to_string()))?;
    let message = message(&manifest, &call)?;
    if let Some(arg) = call.args.first() {
        return Err(syn::Error::new(
            arg.span,
            "msg_id! takes only the id; pass the arguments where the message is formatted",
        ));
    }
    let span = call.id.span();
    let mut raw = Literal::u32_unsuffixed(message.msg_id.raw());
    raw.set_span(span);
    Ok(quote_spanned! {span=> #krate::__mf2::MsgId::from_raw(#raw) })
}

pub(crate) fn expand(input: Input) -> syn::Result<TokenStream> {
    let Input {
        source,
        hash,
        krate,
        call,
    } = input;
    let manifest = crate::manifest::load(&source, hash)
        .map_err(|e| syn::Error::new(call.id.span(), e.to_string()))?;
    let message = message(&manifest, &call)?;
    let sorted = check(&message, &call)?;
    Ok(emit(&krate, &call, &message, &sorted))
}

/// What the manifest says about the message a call site names.
struct Message<'m> {
    id: String,
    msg_id: mf2_catalog::MsgId,
    /// Its variables, in slot order.
    slots: &'m [String],
    /// Its markup names, ascending.
    markup: &'m [String],
}

fn message<'m>(manifest: &'m Arc<Manifest>, call: &Call) -> syn::Result<Message<'m>> {
    let id = call.id.value();
    let Some(msg_id) = manifest.msg_id(&id) else {
        let hint = suggest(&id, manifest.ids.iter().map(String::as_str))
            .map_or_else(String::new, |s| format!("; did you mean `{s}`?"));
        return Err(syn::Error::new(
            call.id.span(),
            format!("unknown message id `{id}`{hint}"),
        ));
    };
    let index = msg_id.index() as usize;
    let empty: &[String] = &[];
    Ok(Message {
        id,
        msg_id,
        slots: manifest.slots.get(index).map_or(empty, Vec::as_slice),
        markup: manifest.markup.get(index).map_or(empty, Vec::as_slice),
    })
}

/// A call's arguments in the message's own order: one entry per slot, one
/// per markup name.
struct Sorted<'c> {
    values: Vec<Option<&'c Expr>>,
    handlers: Vec<Option<&'c Expr>>,
}

/// The call's arguments, sorted into the message's slots and markup names.
/// Every problem of one call site is reported at once.
fn check<'c>(message: &Message<'_>, call: &'c Call) -> syn::Result<Sorted<'c>> {
    let mut values: Vec<Option<&Expr>> = vec![None; message.slots.len()];
    let mut handlers: Vec<Option<&Expr>> = vec![None; message.markup.len()];
    let mut errors: Option<syn::Error> = None;
    let mut push = |e: syn::Error| match &mut errors {
        Some(all) => all.combine(e),
        None => errors = Some(e),
    };

    for arg in &call.args {
        let name = nfc(&arg.name);
        let slot = message.slots.iter().position(|s| *s == name);
        let markup = message.markup.iter().position(|m| *m == name);
        match (slot, markup) {
            (Some(i), _) if values.get(i).is_some_and(Option::is_some) => push(syn::Error::new(
                arg.span,
                format!("argument `{}` is given twice", arg.name),
            )),
            (Some(i), _) => {
                if let Some(slot) = values.get_mut(i) {
                    *slot = Some(&arg.value);
                }
            }
            (None, Some(i)) if handlers.get(i).is_some_and(Option::is_some) => {
                push(syn::Error::new(
                    arg.span,
                    format!("markup handler `{}` is given twice", arg.name),
                ));
            }
            (None, Some(i)) => {
                if let Some(handler) = handlers.get_mut(i) {
                    *handler = Some(&arg.value);
                }
            }
            (None, None) => push(syn::Error::new(arg.span, unknown(message, &arg.name))),
        }
    }

    let missing: Vec<&str> = message
        .slots
        .iter()
        .zip(&values)
        .filter(|(_, v)| v.is_none())
        .map(|(s, _)| s.as_str())
        .collect();
    if !missing.is_empty() {
        push(syn::Error::new(call.id.span(), needs(message, &missing)));
    }

    // Handlers are all or none: a message's markup formats to parts without
    // any (which is what the suite's markup tests assert), but handling one
    // markup name and not its sibling is an oversight (04 §2.1).
    if handlers.iter().any(Option::is_some) {
        let absent: Vec<&str> = message
            .markup
            .iter()
            .zip(&handlers)
            .filter(|(_, h)| h.is_none())
            .map(|(m, _)| m.as_str())
            .collect();
        if !absent.is_empty() {
            push(syn::Error::new(
                call.id.span(),
                format!(
                    "message `{}` has markup {} without a handler; a call site supplies \
                     all of a message's markup handlers or none of them",
                    message.id,
                    list(&absent, "`")
                ),
            ));
        }
        // The handlers are found by the hash of their name, so a message
        // whose markup names collide could dispatch to the wrong one. The
        // macro is where that is knowable — and where it can be said.
        for (i, a) in message.markup.iter().enumerate() {
            for b in message.markup.iter().skip(i + 1) {
                if markup_key(a) == markup_key(b) {
                    push(syn::Error::new(
                        call.id.span(),
                        format!(
                            "the markup names `{a}` and `{b}` of message `{}` have the same \
                             64-bit hash, which is what a call site's handlers are found by; \
                             rename one of them",
                            message.id
                        ),
                    ));
                }
            }
        }
    }

    match errors {
        Some(e) => Err(e),
        None => Ok(Sorted { values, handlers }),
    }
}

/// The positional construction, spanned at the user's id literal so that an
/// error points at the call site rather than into the generated wrapper in
/// someone's `OUT_DIR`; each argument's conversion is spanned at the
/// argument itself, which is where a type that is not an argument is
/// reported.
fn emit(
    krate: &TokenStream,
    call: &Call,
    message: &Message<'_>,
    sorted: &Sorted<'_>,
) -> TokenStream {
    let Sorted { values, handlers } = sorted;
    let span = call.id.span();
    let mut raw = Literal::u32_unsuffixed(message.msg_id.raw());
    raw.set_span(span);
    let id = quote_spanned! {span=> #krate::__mf2::MsgId::from_raw(#raw) };
    let rich = handlers.iter().any(Option::is_some);
    let args: Vec<&Expr> = values.iter().copied().flatten().collect();
    // A string literal keeps its `&'static str`: `IntoArg for &str` has to
    // copy, because a call site's `&str` is rarely `'static` and the
    // description outlives the call (04 §2.1). Any other argument goes
    // through `mf2::__arg`'s dispatch (19 §7): `IntoArg`, else a `From<T>
    // for ArgValue` (1.x's conversion), else its `Display` text, else
    // `IntoArg`'s own message. The value goes straight into `convert`, as it
    // went into `ArgValue::from(e)`: never bound or borrowed here, and its
    // temporaries live as long. The closure picks the step from the type.
    // Our tokens are located at the argument, so that the error points
    // there (on stable a span is one token: the argument's first), with
    // `mixed_site` hygiene, so that the closure's parameter cannot meet the
    // application's names and lints treat them as the macro's.
    let converted = args.iter().any(|e| !is_str_literal(e));
    let value = |e: &&Expr| match e {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => quote_spanned! {span=> #krate::__mf2::ArgValue::str_static(#s) },
        e => {
            let user = syn::spanned::Spanned::span(*e);
            let at = Span::mixed_site().located_at(user);
            let probe = Ident::new("__mf2_probe", at);
            // The method a type with none of the forms fails at, with the
            // argument's own span: the error is then the application's line
            // alone, not "originates in the macro".
            let kind = Ident::new("__mf2_kind", user);
            quote_spanned! {at=>
                #krate::__mf2::__arg::convert(#e, |#probe| (&&&#probe).#kind())
            }
        }
    };

    // `tr` for a plain message, an arity-specific constructor for up to four
    // arguments — the reference workload's maximum — and only beyond that a
    // constructor generic over the array length (04 §2).
    let description = match args.as_slice() {
        [] if !rich => return quote_spanned! {span=> #krate::__mf2::tr(#id) },
        [] => quote_spanned! {span=> #krate::__mf2::tr_args0(#id) },
        [a] => {
            let a = value(a);
            quote_spanned! {span=> #krate::__mf2::tr_args1(#id, #a) }
        }
        [a, b] => {
            let (a, b) = (value(a), value(b));
            quote_spanned! {span=> #krate::__mf2::tr_args2(#id, #a, #b) }
        }
        [a, b, c] => {
            let (a, b, c) = (value(a), value(b), value(c));
            quote_spanned! {span=> #krate::__mf2::tr_args3(#id, #a, #b, #c) }
        }
        [a, b, c, d] => {
            let (a, b, c, d) = (value(a), value(b), value(c), value(d));
            quote_spanned! {span=> #krate::__mf2::tr_args4(#id, #a, #b, #c, #d) }
        }
        many => {
            let values = many.iter().map(value);
            quote_spanned! {span=> #krate::__mf2::tr_args_n(#id, [#(#values),*].into()) }
        }
    };
    // The dispatch's steps are trait methods, so the traits are in scope for
    // the whole description, and nowhere else.
    let description = if converted {
        quote_spanned! {span=>
            {
                #[allow(unused_imports)]
                use #krate::__mf2::__arg::{
                    KindDisplay as _, KindFrom as _, KindIntoArg as _, KindNeither as _,
                };
                #description
            }
        }
    } else {
        description
    };
    if !rich {
        return description;
    }
    let entries = message
        .markup
        .iter()
        .zip(handlers)
        .filter_map(|(name, handler)| {
            let handler = (*handler)?;
            let mut key = Literal::u64_suffixed(markup_key(name));
            key.set_span(span);
            // A closure goes through `markup_view`, whose `Fn` bound gives
            // it its argument's type: `|c| view! { … }` needs no `c: AnyView`.
            Some(if matches!(handler, Expr::Closure(_)) {
                quote_spanned! {span=> (#key, #krate::__mf2::markup_view(#handler)) }
            } else {
                quote_spanned! {span=> (#key, #krate::__mf2::markup(#handler)) }
            })
        });
    quote_spanned! {span=> #krate::__mf2::tr_rich(#description, [#(#entries),*].into()) }
}

/// A string literal, which becomes `ArgValue::str_static` with no dispatch.
fn is_str_literal(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(_),
            ..
        })
    )
}

/// The message for a name the message does not have. A name can be either
/// kind — an argument or a markup handler — so the suggestion says which
/// kind it found.
fn unknown(message: &Message<'_>, name: &str) -> String {
    let hint = match suggest(name, message.slots.iter().map(String::as_str)) {
        Some(s) => format!("; did you mean `{s}`?"),
        None => suggest(name, message.markup.iter().map(String::as_str))
            .map_or_else(String::new, |s| {
                format!("; did you mean the markup handler `{s}`?")
            }),
    };
    let has = if message.slots.is_empty() {
        "it has no variables".to_owned()
    } else {
        format!("its variables: {}", list(message.slots, "$"))
    };
    let markup = if message.markup.is_empty() {
        String::new()
    } else {
        format!("; its markup: {}", list(message.markup, "#"))
    };
    let what = if message.markup.is_empty() {
        format!("has no variable `${name}`")
    } else {
        format!("has no variable `${name}` and no markup `#{name}`")
    };
    format!("message `{}` {what} ({has}{markup}){hint}", message.id)
}

/// The message for arguments the call site did not pass.
fn needs(message: &Message<'_>, missing: &[&str]) -> String {
    let all = message
        .slots
        .iter()
        .map(|v| format!("{v} = …"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "message `{}` needs argument{} {} (its variables: {}); write tr!(\"{}\", {all})",
        message.id,
        if missing.len() == 1 { "" } else { "s" },
        list(missing, "`"),
        list(message.slots, "$"),
        message.id,
    )
}

/// `$a, $b` / `` `a`, `b` `` — names with a sigil, for a message.
fn list<S: AsRef<str>>(names: &[S], sigil: &str) -> String {
    if names.is_empty() {
        return "none".to_owned();
    }
    names
        .iter()
        .map(|n| {
            if sigil == "`" {
                format!("`{}`", n.as_ref())
            } else {
                format!("{sigil}{}", n.as_ref())
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The closest candidate within a third of the name's length (at least 2
/// edits), for a did-you-mean. Ties go to the candidate of the most similar
/// length — `kdb` is two edits from both `kbd` and `b`, and only one of
/// those is what anyone meant.
fn suggest<'a>(wanted: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let len = wanted.chars().count();
    let limit = (len / 3).max(2);
    candidates
        .map(|c| (distance(wanted, c), c.chars().count().abs_diff(len), c))
        .filter(|(d, _, _)| *d <= limit)
        .min()
        .map(|(_, _, c)| c)
}

/// Levenshtein distance.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        if let Some(first) = cur.first_mut() {
            *first = i + 1;
        }
        for (j, cb) in b.iter().enumerate() {
            let sub = prev.get(j).copied().unwrap_or(0) + usize::from(ca != *cb);
            let del = prev.get(j + 1).copied().unwrap_or(0) + 1;
            let ins = cur.get(j).copied().unwrap_or(0) + 1;
            if let Some(slot) = cur.get_mut(j + 1) {
                *slot = sub.min(del).min(ins);
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev.last().copied().unwrap_or(0)
}

/// An argument name in NFC — what the manifest's names are (05 §3). A Rust
/// identifier already is one; a string-literal name need not be.
fn nfc(name: &str) -> std::borrow::Cow<'_, str> {
    use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};
    match is_nfc_quick(name.chars()) {
        IsNormalized::Yes => std::borrow::Cow::Borrowed(name),
        IsNormalized::No => std::borrow::Cow::Owned(name.nfc().collect()),
        IsNormalized::Maybe => {
            let normalized: String = name.nfc().collect();
            if normalized == name {
                std::borrow::Cow::Borrowed(name)
            } else {
                std::borrow::Cow::Owned(normalized)
            }
        }
    }
}
