//! A `tr!` / `move_tr!` call's arguments, in the forms `leptos-fluent`
//! 0.3.1's `macro_rules!` accept (the tooling design §6.2):
//!
//! ```text
//! "id"                          "id", { "name" => value, … }
//! context, "id"                 context, "id", { … }
//! #[cfg(…)] …                   if c { "a" } else { "b" }, …
//! id_expression, …
//! ```
//!
//! Only the first two lines are rewritten; the others are named, so that
//! the report can say which form a call is.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use syn::parse::{ParseStream, Parser};
use syn::{Expr, Lit, LitStr, Token};

/// One `"name" => value` of an argument map.
pub(crate) struct Arg {
    /// The name, unquoted.
    pub(crate) name: String,
    pub(crate) value: Expr,
}

/// A call the rules can rewrite.
pub(crate) struct Call {
    /// Whether the context came first (`tr!(i18n, "id")`).
    pub(crate) context: bool,
    pub(crate) id: LitStr,
    /// The argument map, if one was written (it may be empty).
    pub(crate) args: Option<Vec<Arg>>,
}

/// What a call's arguments are.
pub(crate) enum Form {
    Call(Call),
    /// The id is an expression, not a literal.
    DynamicId(Span),
    /// `tr!(if c { "a" } else { "b" })`.
    If(Span),
    /// `#[cfg(…)]` on the id or the map.
    Cfg(Span),
    /// A map key that is not a string literal.
    ArgumentName(Span),
    /// None of the forms.
    Unparsed(Span),
}

/// A top-level segment of the arguments, between commas.
enum Segment {
    Id(LitStr),
    Map(proc_macro2::Group),
    Other(Span),
}

pub(crate) fn parse(group: &proc_macro2::Group) -> Form {
    let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
    let Some(first) = tokens.first() else {
        return Form::Unparsed(group.span());
    };
    let mut segments: Vec<Vec<TokenTree>> = vec![Vec::new()];
    for token in &tokens {
        match token {
            TokenTree::Punct(p) if p.as_char() == '#' => return Form::Cfg(p.span()),
            TokenTree::Punct(p) if p.as_char() == ',' => segments.push(Vec::new()),
            other => {
                if let Some(last) = segments.last_mut() {
                    last.push(other.clone());
                }
            }
        }
    }
    if segments.last().is_some_and(Vec::is_empty) {
        segments.pop();
    }
    if segments.iter().any(|s| is_if(s)) {
        let at = segments
            .iter()
            .find(|s| is_if(s))
            .and_then(|s| s.first())
            .map_or(first.span(), TokenTree::span);
        return Form::If(at);
    }
    let classified: Vec<Segment> = segments.iter().map(|s| classify(s)).collect();
    let (context, id, map) = match classified.as_slice() {
        [Segment::Id(id)] => (false, id, None),
        [Segment::Id(id), Segment::Map(map)] => (false, id, Some(map)),
        [Segment::Other(_), Segment::Id(id)] => (true, id, None),
        [Segment::Other(_), Segment::Id(id), Segment::Map(map)] => (true, id, Some(map)),
        [Segment::Other(at)]
        | [Segment::Other(at), Segment::Map(_)]
        | [Segment::Other(_), Segment::Other(at)]
        | [Segment::Other(_), Segment::Other(at), Segment::Map(_)] => {
            return Form::DynamicId(*at);
        }
        _ => return Form::Unparsed(first.span()),
    };
    let args = match map {
        None => None,
        Some(map) => match parse_map(map) {
            Ok(args) => Some(args),
            Err(form) => return form,
        },
    };
    Form::Call(Call {
        context,
        id: id.clone(),
        args,
    })
}

fn is_if(segment: &[TokenTree]) -> bool {
    matches!(segment.first(), Some(TokenTree::Ident(i)) if i == "if")
}

fn classify(segment: &[TokenTree]) -> Segment {
    match segment {
        [TokenTree::Literal(l)] => match Lit::new(l.clone()) {
            Lit::Str(s) => Segment::Id(s),
            _ => Segment::Other(l.span()),
        },
        [TokenTree::Group(g)] if g.delimiter() == Delimiter::Brace => Segment::Map(g.clone()),
        [first, ..] => Segment::Other(first.span()),
        [] => Segment::Other(Span::call_site()),
    }
}

/// `{ "name" => value, … }`.
fn parse_map(map: &proc_macro2::Group) -> Result<Vec<Arg>, Form> {
    for token in map.stream() {
        if let TokenTree::Punct(p) = &token
            && p.as_char() == '#'
        {
            return Err(Form::Cfg(p.span()));
        }
    }
    let parser = |input: ParseStream<'_>| -> syn::Result<Result<Vec<Arg>, Form>> {
        let mut args = Vec::new();
        while !input.is_empty() {
            let key: Lit = input.parse()?;
            input.parse::<Token![=>]>()?;
            let value: Expr = input.parse()?;
            let name = match &key {
                Lit::Str(s) => s.value(),
                other => return Ok(Err(Form::ArgumentName(other.span()))),
            };
            args.push(Arg { name, value });
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Ok(args))
    };
    match parser.parse2(map.stream()) {
        Ok(result) => result,
        Err(e) => Err(Form::Unparsed(e.span())),
    }
}

/// Whether evaluating `e` again gives the same value: a literal, a path, or
/// a reference to one (§6.2, *constant*).
pub(crate) fn constant(e: &Expr) -> bool {
    match e {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Reference(r) => constant(&r.expr),
        Expr::Paren(p) => constant(&p.expr),
        Expr::Group(g) => constant(&g.expr),
        Expr::Unary(u) => matches!(u.op, syn::UnOp::Neg(_)) && matches!(*u.expr, Expr::Lit(_)),
        _ => false,
    }
}

/// The byte range of a value in its file.
pub(crate) fn range(tokens: TokenStream) -> Option<std::ops::Range<usize>> {
    let mut iter = tokens.into_iter();
    let first = iter.next()?;
    let last = iter.last().unwrap_or_else(|| first.clone());
    Some(first.span().byte_range().start..last.span().byte_range().end)
}
