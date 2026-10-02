//! The call-site rules of the tooling design §6.2, as byte-range edits of
//! one Rust file.
//!
//! The file is tokenized with `proc-macro2` (span locations on, so every
//! token knows its bytes), which reaches into every macro's arguments —
//! `view!` included — where `syn`'s syntax tree would stop. Each `tr!` or
//! `move_tr!` is classified by where it stands (a *view position* or a
//! *`String` position*) and replaced by the text of its rule; nothing else
//! in the file is touched.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::Range;
use std::path::Path;

use proc_macro2::{Delimiter, Group, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::UseTree;

use super::call::{self, Call, Form};
use crate::convert::report::{Code, Finding};

/// A rule of §6.2, named as the plan and the tests name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    TrString,
    TrView,
    MoveTrView,
    MoveTrSignal,
    Closure,
    Arguments,
    Context,
    Attribute,
    Import,
}

impl Rule {
    /// Every rule.
    /// (`src/workspace_tests.rs` holds it against the plan.)
    #[cfg(all(test, mf2_workspace))]
    pub(crate) const ALL: [Rule; 9] = [
        Rule::TrString,
        Rule::TrView,
        Rule::MoveTrView,
        Rule::MoveTrSignal,
        Rule::Closure,
        Rule::Arguments,
        Rule::Context,
        Rule::Attribute,
        Rule::Import,
    ];

    /// The rule's name in §6.2.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Rule::TrString => "tr-string",
            Rule::TrView => "tr-view",
            Rule::MoveTrView => "move-tr-view",
            Rule::MoveTrSignal => "move-tr-signal",
            Rule::Closure => "closure",
            Rule::Arguments => "arguments",
            Rule::Context => "context",
            Rule::Attribute => "attribute",
            Rule::Import => "import",
        }
    }
}

/// The converted messages of the source locale: id → its variables.
#[derive(Debug, Default)]
pub(crate) struct Messages(pub(crate) BTreeMap<String, BTreeSet<String>>);

/// How a file is rewritten.
#[derive(Debug, Default)]
pub(crate) struct Options {
    /// The i18n crate, as a path segment (`my_app_i18n`).
    pub(crate) i18n_crate: Option<String>,
    /// A rule switched off — the negative control: the calls it would
    /// rewrite are reported instead.
    pub(crate) without: Option<Rule>,
}

/// Where a call stands (§6.2, *Positions*).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    View,
    String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Macro {
    Tr,
    MoveTr,
}

/// A `[leptos_fluent::](tr|move_tr)!(…)` in the token stream.
struct Site {
    /// The first byte: the path's, or the macro name's.
    start: usize,
    name: Span,
    which: Macro,
    group: Group,
    qualified: bool,
}

/// `[move] || <site>`.
struct ClosureSite {
    start: usize,
    site: Site,
}

struct Edit {
    range: Range<usize>,
    text: String,
}

/// What rewriting one file gave.
pub(crate) struct Rewritten {
    /// The new text, if anything changed.
    pub(crate) text: Option<String>,
    pub(crate) findings: Vec<Finding>,
}

/// Rewrites one file by §6.2.
pub(crate) fn rewrite(file: &Path, src: &str, messages: &Messages, options: &Options) -> Rewritten {
    let mut walker = Walker {
        src,
        file,
        messages,
        options,
        findings: Vec::new(),
        edits: Vec::new(),
    };
    let tokens: Vec<TokenTree> = match src.parse::<TokenStream>() {
        Ok(stream) => stream.into_iter().collect(),
        Err(e) => {
            walker.report(
                Code::LfParse,
                e.span(),
                None,
                format!("the file does not tokenize ({e}); nothing in it was rewritten"),
            );
            return walker.finish();
        }
    };
    if names_leptos_fluent(&tokens) {
        walker.walk(&tokens, false);
    } else {
        // Not `leptos-fluent`'s `tr!`; but a `move_tr!` can only be its,
        // come through a re-export.
        walker.stray_move_tr(&tokens);
    }
    walker.finish()
}

/// The `locales: "…"` of a `leptos_fluent!` or `static_loader!` in `src`.
pub(crate) fn initializer_locales(src: &str) -> Option<String> {
    let tokens: Vec<TokenTree> = src.parse::<TokenStream>().ok()?.into_iter().collect();
    find_locales(&tokens)
}

fn find_locales(tokens: &[TokenTree]) -> Option<String> {
    for (i, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(id)
                if (id == "leptos_fluent" || id == "static_loader")
                    && bang_group(tokens, i + 1).is_some() =>
            {
                let group = bang_group(tokens, i + 1)?;
                let inner: Vec<TokenTree> = group.stream().into_iter().collect();
                if let Some(found) = string_field(&inner, "locales") {
                    return Some(found);
                }
                if let Some(found) = find_locales(&inner) {
                    return Some(found);
                }
            }
            TokenTree::Group(g) => {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                if let Some(found) = find_locales(&inner) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// The string literal after `key:` among an initializer's `tokens`.
fn string_field(tokens: &[TokenTree], key: &str) -> Option<String> {
    tokens.iter().enumerate().find_map(|(j, t)| match t {
        TokenTree::Ident(name) if name == key && punct(tokens.get(j + 1), ':') => {
            match tokens.get(j + 2) {
                Some(TokenTree::Literal(l)) => match syn::Lit::new(l.clone()) {
                    syn::Lit::Str(s) => Some(s.value()),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    })
}

fn names_leptos_fluent(tokens: &[TokenTree]) -> bool {
    tokens.iter().any(|t| match t {
        TokenTree::Ident(id) => id == "leptos_fluent",
        TokenTree::Group(g) => names_leptos_fluent(&g.stream().into_iter().collect::<Vec<_>>()),
        _ => false,
    })
}

fn punct(token: Option<&TokenTree>, c: char) -> bool {
    matches!(token, Some(TokenTree::Punct(p)) if p.as_char() == c)
}

fn ident(token: Option<&TokenTree>, name: &str) -> bool {
    matches!(token, Some(TokenTree::Ident(i)) if i == name)
}

/// `! (…)` at `i`.
fn bang_group(tokens: &[TokenTree], i: usize) -> Option<&Group> {
    if !punct(tokens.get(i), '!') {
        return None;
    }
    match tokens.get(i + 1) {
        Some(TokenTree::Group(g)) => Some(g),
        _ => None,
    }
}

/// `::` at `i`.
fn path_sep(tokens: &[TokenTree], i: usize) -> bool {
    punct(tokens.get(i), ':') && punct(tokens.get(i + 1), ':')
}

fn start(token: &TokenTree) -> usize {
    token.span().byte_range().start
}

/// A call at `i`, and the index after it.
fn site_at(tokens: &[TokenTree], i: usize) -> Option<(Site, usize)> {
    let (qualified, at) = if ident(tokens.get(i), "leptos_fluent") && path_sep(tokens, i + 1) {
        (true, i + 3)
    } else {
        // `other::tr!` is someone else's macro.
        if i >= 1 && punct(tokens.get(i - 1), ':') {
            return None;
        }
        (false, i)
    };
    let which = match tokens.get(at) {
        Some(TokenTree::Ident(id)) if id == "tr" => Macro::Tr,
        Some(TokenTree::Ident(id)) if id == "move_tr" => Macro::MoveTr,
        _ => return None,
    };
    let group = bang_group(tokens, at + 1)?.clone();
    Some((
        Site {
            start: start(&tokens[i]),
            name: tokens[at].span(),
            which,
            group,
            qualified,
        },
        at + 3,
    ))
}

/// `[move] || <call>` at `i`.
fn closure_at(tokens: &[TokenTree], i: usize) -> Option<(ClosureSite, usize)> {
    let bars = if ident(tokens.get(i), "move") {
        i + 1
    } else {
        i
    };
    if !(punct(tokens.get(bars), '|') && punct(tokens.get(bars + 1), '|')) {
        return None;
    }
    let (site, next) = site_at(tokens, bars + 2)?;
    Some((
        ClosureSite {
            start: start(&tokens[i]),
            site,
        },
        next,
    ))
}

/// Whether the tokens after a call continue its expression (`.x`, `?`), so
/// that the value rendered is not the call's own.
fn continued(rest: &[TokenTree]) -> bool {
    punct(rest.first(), '.') || punct(rest.first(), '?')
}

fn followed_by_to_string(rest: &[TokenTree]) -> bool {
    punct(rest.first(), '.')
        && ident(rest.get(1), "to_string")
        && matches!(rest.get(2), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis && g.stream().is_empty())
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name != "_"
}

struct Walker<'a> {
    src: &'a str,
    file: &'a Path,
    messages: &'a Messages,
    options: &'a Options,
    findings: Vec<Finding>,
    edits: Vec<Edit>,
}

impl Walker<'_> {
    fn finish(mut self) -> Rewritten {
        let text = if self.edits.is_empty() {
            None
        } else {
            let edits = std::mem::take(&mut self.edits);
            Some(apply(self.src, 0, edits))
        };
        Rewritten {
            text,
            findings: self.findings,
        }
    }

    fn report(&mut self, code: Code, span: Span, id: Option<String>, message: String) {
        let at = span.start();
        self.findings.push(Finding {
            code,
            locale: String::new(),
            file: self.file.to_path_buf(),
            line: u32::try_from(at.line).unwrap_or(u32::MAX),
            column: u32::try_from(at.column + 1).unwrap_or(u32::MAX),
            id,
            message,
        });
    }

    fn disabled(&self, rules: &[Rule]) -> Option<Rule> {
        self.options.without.filter(|r| rules.contains(r))
    }

    fn walk(&mut self, tokens: &[TokenTree], view: bool) {
        let mut i = 0;
        while i < tokens.len() {
            if let Some(next) = self.use_item(tokens, i) {
                i = next;
                continue;
            }
            let after_eq = view && i >= 1 && punct(tokens.get(i - 1), '=');
            if let Some((site, next)) = site_at(tokens, i) {
                let position = if after_eq && !continued(&tokens[next..]) {
                    Position::View
                } else {
                    Position::String
                };
                self.call(&site, position, &tokens[next..]);
                i = next;
                continue;
            }
            if after_eq
                && let Some((closure, next)) = closure_at(tokens, i)
                && !continued(&tokens[next..])
                && self.closure(&closure)
            {
                i = next;
                continue;
            }
            match &tokens[i] {
                TokenTree::Ident(id)
                    if (id == "leptos_fluent" || id == "static_loader")
                        && bang_group(tokens, i + 1).is_some() =>
                {
                    self.initializer(id.span(), &id.to_string(), bang_group(tokens, i + 1));
                    i += 3;
                    continue;
                }
                TokenTree::Ident(id) if id == "leptos_fluent" && path_sep(tokens, i + 1) => {
                    match tokens.get(i + 3) {
                        Some(TokenTree::Ident(x))
                            if x == "leptos_fluent" && bang_group(tokens, i + 4).is_some() =>
                        {
                            self.initializer(id.span(), "leptos_fluent", bang_group(tokens, i + 4));
                            i += 6;
                        }
                        Some(x) => {
                            let what = x.to_string();
                            self.report(
                                Code::LfContext,
                                x.span(),
                                None,
                                format!("`leptos_fluent::{what}` has no mf2 counterpart the rewrite can know"),
                            );
                            i += 4;
                        }
                        None => i += 1,
                    }
                    continue;
                }
                TokenTree::Ident(id) if id == "I18n" => {
                    self.report(
                        Code::LfContext,
                        id.span(),
                        None,
                        "the `leptos-fluent` context: its language and languages become \
                         `<LocaleSwitcher>` or the locale API, its `tr` / `tr_with_args` \
                         `msg_id!` and `TrDyn`"
                            .to_owned(),
                    );
                }
                TokenTree::Ident(id) if id == "view" => {
                    if let Some(group) = bang_group(tokens, i + 1) {
                        let inner: Vec<TokenTree> = group.stream().into_iter().collect();
                        self.walk(&inner, true);
                        i += 3;
                        continue;
                    }
                }
                TokenTree::Group(g) => {
                    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                    // A block of a view whose whole content is the call is
                    // a view position; a closure's body (`|_| { … }`) is not.
                    let block = view
                        && g.delimiter() == Delimiter::Brace
                        && !(i >= 1 && punct(tokens.get(i - 1), '|'));
                    if block {
                        if let Some((site, next)) = site_at(&inner, 0)
                            && next == inner.len()
                        {
                            self.call(&site, Position::View, &[]);
                            i += 1;
                            continue;
                        }
                        if let Some((closure, next)) = closure_at(&inner, 0)
                            && next == inner.len()
                            && self.closure(&closure)
                        {
                            i += 1;
                            continue;
                        }
                    }
                    self.walk(&inner, false);
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// In a file that does not name `leptos_fluent`: report each `move_tr!`.
    fn stray_move_tr(&mut self, tokens: &[TokenTree]) {
        for (i, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Ident(id) if id == "move_tr" && bang_group(tokens, i + 1).is_some() => {
                    self.report(
                        Code::LfImport,
                        id.span(),
                        None,
                        "this file's `move_tr!` comes through a re-export; import \
                         `leptos_fluent`'s macros in it directly and run again"
                            .to_owned(),
                    );
                }
                TokenTree::Group(g) => {
                    self.stray_move_tr(&g.stream().into_iter().collect::<Vec<_>>());
                }
                _ => {}
            }
        }
    }

    fn initializer(&mut self, span: Span, name: &str, group: Option<&Group>) {
        let mut message = format!(
            "`{name}!` initializes leptos-fluent: replace it with the generated \
             `install()`, on the server and in the browser"
        );
        // mf2's client writes its own cookie on every switch, so the old
        // one can only be read, as an extra source, never renamed.
        let inner: Vec<TokenTree> = group
            .map(|g| g.stream().into_iter().collect())
            .unwrap_or_default();
        if let Some(cookie) = string_field(&inner, "cookie_name") {
            let _ = write!(
                message,
                "; its `{cookie}` cookie is not read: the client always writes `mf2_locale`, \
                 so to keep the language readers chose before, add \
                 `CookieLocale {{ name: {cookie:?}, ..Default::default() }}` to the server's \
                 `Negotiator` as an extra source, after `CookieLocale::default()` and before \
                 `AcceptLanguage`"
            );
        }
        self.report(Code::LfInitializer, span, None, message);
    }

    /// `use leptos_fluent::…;` at `i`: rewritten, or reported. Returns the
    /// index after it.
    fn use_item(&mut self, tokens: &[TokenTree], i: usize) -> Option<usize> {
        if !ident(tokens.get(i), "use") {
            return None;
        }
        let path = if path_sep(tokens, i + 1) {
            i + 3
        } else {
            i + 1
        };
        if !ident(tokens.get(path), "leptos_fluent") {
            return None;
        }
        let end = (path..tokens.len()).find(|&j| punct(tokens.get(j), ';'))?;
        let public = (i >= 1 && ident(tokens.get(i - 1), "pub"))
            || (i >= 2
                && ident(tokens.get(i - 2), "pub")
                && matches!(tokens.get(i - 1), Some(TokenTree::Group(_))));
        let stream: TokenStream = tokens[i..=end].iter().cloned().collect();
        let mut names = Vec::new();
        let mut plain = true;
        match syn::parse2::<syn::ItemUse>(stream) {
            Ok(item) => leaves(&item.tree, &mut names, &mut plain),
            Err(_) => plain = false,
        }
        let only_macros = plain && names.iter().all(|n| n == "tr" || n == "move_tr");
        let span = tokens[i].span();
        let range = start(&tokens[i])..tokens[end].span().byte_range().end;
        match (&self.options.i18n_crate, public, only_macros) {
            (Some(krate), false, true) => {
                if self.disabled(&[Rule::Import]).is_some() {
                    self.report(
                        Code::LfCall,
                        span,
                        None,
                        "the rule `import` would rewrite this `use`; it is disabled".to_owned(),
                    );
                } else {
                    self.edits.push(Edit {
                        range,
                        text: format!("use {krate}::tr;"),
                    });
                }
            }
            (krate, public, _) => {
                let why = if !only_macros {
                    format!("it names {}", names.join(", "))
                } else if public {
                    "it re-exports them".to_owned()
                } else if krate.is_none() {
                    "the i18n crate's name is unknown (--i18n-crate, or `--dir` holding its Cargo.toml)"
                        .to_owned()
                } else {
                    String::new()
                };
                self.report(
                    Code::LfImport,
                    span,
                    None,
                    format!("this `use leptos_fluent` is not rewritten: {why}"),
                );
            }
        }
        Some(end + 1)
    }

    /// The closure rule; `false` when it does not apply, so that the call in
    /// the closure's body is taken as a `String` position.
    fn closure(&mut self, closure: &ClosureSite) -> bool {
        let site = &closure.site;
        if site.which != Macro::Tr {
            return false;
        }
        let Form::Call(call) = call::parse(&site.group) else {
            return false;
        };
        let constant = call.args.iter().flatten().all(|a| call::constant(&a.value));
        if !constant {
            return false;
        }
        if let Some(rule) = self.disabled(&[Rule::Closure]) {
            self.report(
                Code::LfCall,
                site.name,
                Some(call.id.value()),
                format!(
                    "the rule `{}` would rewrite this closure; it is disabled",
                    rule.name()
                ),
            );
            return true;
        }
        let Some(text) = self.render(site, &call, Position::View, &[]) else {
            return true;
        };
        self.edits.push(Edit {
            range: closure.start..site.group.span().byte_range().end,
            text,
        });
        true
    }

    fn call(&mut self, site: &Site, position: Position, rest: &[TokenTree]) {
        let call = match call::parse(&site.group) {
            Form::Call(call) => call,
            Form::DynamicId(span) => {
                return self.report(
                    Code::LfDynamicId,
                    span,
                    None,
                    "the id is not a string literal: use a literal id, or `msg_id!` and `TrDyn`"
                        .to_owned(),
                );
            }
            Form::If(span) => {
                return self.report(
                    Code::LfIfForm,
                    span,
                    None,
                    "write `if c { tr!(\"a\") } else { tr!(\"b\") }`, each branch the same type"
                        .to_owned(),
                );
            }
            Form::Cfg(span) => {
                return self.report(
                    Code::LfCfg,
                    span,
                    None,
                    "`#[cfg]` inside the call: put it on a statement around the call".to_owned(),
                );
            }
            Form::ArgumentName(span) => {
                return self.report(
                    Code::LfArgumentName,
                    span,
                    None,
                    "an argument name must be a string literal".to_owned(),
                );
            }
            Form::Unparsed(span) => {
                return self.report(
                    Code::LfCall,
                    span,
                    None,
                    "the call's arguments are none of the forms leptos-fluent documents".to_owned(),
                );
            }
        };
        if let Some(text) = self.render(site, &call, position, rest) {
            self.edits.push(Edit {
                range: site.start..site.group.span().byte_range().end,
                text,
            });
        }
    }

    /// The call's replacement, or `None` when a disabled rule leaves it
    /// reported.
    fn render(
        &mut self,
        site: &Site,
        call: &Call,
        position: Position,
        rest: &[TokenTree],
    ) -> Option<String> {
        let id = call.id.value();
        let constant = call.args.iter().flatten().all(|a| call::constant(&a.value));
        let rule = match (site.which, position) {
            (Macro::Tr, Position::String) => Rule::TrString,
            (Macro::Tr, Position::View) => Rule::TrView,
            (Macro::MoveTr, Position::View) if constant => Rule::MoveTrView,
            (Macro::MoveTr, _) => Rule::MoveTrSignal,
        };
        let mut rules = vec![rule];
        if call.context {
            rules.push(Rule::Context);
        }
        if call.args.as_ref().is_some_and(|a| !a.is_empty()) {
            rules.push(Rule::Arguments);
        }
        if id.contains('.') {
            rules.push(Rule::Attribute);
        }
        if site.qualified {
            rules.push(Rule::Import);
        }
        if let Some(disabled) = self.disabled(&rules) {
            self.report(
                Code::LfCall,
                site.name,
                Some(id),
                format!(
                    "the rule `{}` would rewrite this call; it is disabled",
                    disabled.name()
                ),
            );
            return None;
        }
        self.check(site, call);

        let prefix = match (&self.options.i18n_crate, site.qualified) {
            (_, false) => String::new(),
            (Some(krate), true) => format!("{krate}::"),
            (None, true) => {
                self.report(
                    Code::LfImport,
                    site.name,
                    Some(id.clone()),
                    "`leptos_fluent::` is dropped from the call, but the i18n crate's name is unknown"
                        .to_owned(),
                );
                String::new()
            }
        };
        let id_range = call.id.span().byte_range();
        let mut text = format!("{prefix}tr!({}", &self.src[id_range]);
        for arg in call.args.iter().flatten() {
            let value = self.value(&arg.value);
            let _ = if is_identifier(&arg.name) {
                write!(text, ", {} = {value}", arg.name)
            } else {
                write!(text, ", {:?} = {value}", arg.name)
            };
        }
        text.push(')');
        Some(match rule {
            Rule::TrString if followed_by_to_string(rest) => text,
            Rule::TrString => format!("{text}.to_string()"),
            Rule::MoveTrSignal => format!("Signal::derive(move || {text}.to_string())"),
            _ => text,
        })
    }

    /// A value's source text, with any call inside it rewritten too.
    fn value(&mut self, value: &syn::Expr) -> String {
        let tokens = value.to_token_stream();
        let Some(range) = call::range(tokens.clone()) else {
            return String::new();
        };
        let saved = std::mem::take(&mut self.edits);
        let list: Vec<TokenTree> = tokens.into_iter().collect();
        self.walk(&list, false);
        let inner = std::mem::replace(&mut self.edits, saved);
        apply(&self.src[range.clone()], range.start, inner)
    }

    /// The call against the converted messages (§6.2, *Checked*).
    fn check(&mut self, site: &Site, call: &Call) {
        let id = call.id.value();
        let Some(variables) = self.messages.0.get(&id) else {
            self.report(
                Code::LfUnknownId,
                call.id.span(),
                Some(id.clone()),
                format!(
                    "`{id}` is not a message of the converted source locale; `tr!` will not compile"
                ),
            );
            return;
        };
        let given: BTreeSet<String> = call.args.iter().flatten().map(|a| a.name.clone()).collect();
        let extra: Vec<&String> = given.difference(variables).collect();
        let missing: Vec<&String> = variables.difference(&given).collect();
        if extra.is_empty() && missing.is_empty() {
            return;
        }
        let mut parts = Vec::new();
        if !extra.is_empty() {
            parts.push(format!(
                "not a variable of the message: {}",
                extra
                    .iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !missing.is_empty() {
            parts.push(format!(
                "no argument for: {}",
                missing
                    .iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        self.report(
            Code::LfArguments,
            site.name,
            Some(id),
            format!(
                "{}; `tr!` takes exactly the message's variables",
                parts.join("; ")
            ),
        );
    }
}

/// The names a `use` tree imports; `plain` turns false on a rename or glob.
fn leaves(tree: &UseTree, names: &mut Vec<String>, plain: &mut bool) {
    match tree {
        UseTree::Path(p) => leaves(&p.tree, names, plain),
        UseTree::Name(n) => names.push(n.ident.to_string()),
        UseTree::Rename(r) => {
            names.push(format!("{} as {}", r.ident, r.rename));
            *plain = false;
        }
        UseTree::Glob(_) => {
            names.push("*".to_owned());
            *plain = false;
        }
        UseTree::Group(g) => {
            for item in &g.items {
                leaves(item, names, plain);
            }
        }
    }
}

/// `text` (which starts at byte `offset` of its file) with `edits` applied.
fn apply(text: &str, offset: usize, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| e.range.start);
    let mut out = String::with_capacity(text.len() + edits.len() * 16);
    let mut at = 0;
    for edit in edits {
        let from = edit.range.start - offset;
        if from < at {
            continue;
        }
        out.push_str(&text[at..from]);
        out.push_str(&edit.text);
        at = edit.range.end - offset;
    }
    out.push_str(&text[at..]);
    out
}
