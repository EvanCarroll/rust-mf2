//! The MF2 data model (spec/data-model), reduced to what a lossless catalog
//! must carry. Literal quotedness is not data-model information (only the value
//! is), so it is not represented.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Pattern { decls: Vec<Decl>, pattern: Pattern },
    Select { decls: Vec<Decl>, selectors: Vec<String>, variants: Vec<Variant> },
}

impl Message {
    pub fn decls(&self) -> &[Decl] {
        match self {
            Message::Pattern { decls, .. } | Message::Select { decls, .. } => decls,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decl {
    /// `.input {$name …}` — the expression's operand is always `$name`.
    Input { name: String, expr: Expr },
    /// `.local $name = {…}`.
    Local { name: String, expr: Expr },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub keys: Vec<Key>,
    pub pattern: Pattern,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Literal(String),
    CatchAll,
}

pub type Pattern = Vec<Part>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Expr(Expr),
    Markup(Markup),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub operand: Option<Operand>,
    pub function: Option<FunctionRef>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    Literal(String),
    Variable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionRef {
    pub name: String,
    pub options: Vec<(String, Operand)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkupKind {
    Open,
    Standalone,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markup {
    pub kind: MarkupKind,
    pub name: String,
    pub options: Vec<(String, Operand)>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

/// Walks every variable reference and declaration of a message.
pub fn visit_vars(msg: &Message, mut f: impl FnMut(&str)) {
    fn operand(o: &Operand, f: &mut impl FnMut(&str)) {
        if let Operand::Variable(v) = o {
            f(v);
        }
    }
    fn expr(e: &Expr, f: &mut impl FnMut(&str)) {
        if let Some(o) = &e.operand {
            operand(o, f);
        }
        if let Some(func) = &e.function {
            for (_, v) in &func.options {
                operand(v, f);
            }
        }
    }
    fn pattern(p: &Pattern, f: &mut impl FnMut(&str)) {
        for part in p {
            match part {
                Part::Text(_) => {}
                Part::Expr(e) => expr(e, f),
                Part::Markup(m) => {
                    for (_, v) in &m.options {
                        operand(v, f);
                    }
                }
            }
        }
    }
    for d in msg.decls() {
        match d {
            Decl::Input { name, expr: e } => {
                f(name);
                expr(e, &mut f);
            }
            Decl::Local { expr: e, .. } => expr(e, &mut f),
        }
    }
    match msg {
        Message::Pattern { pattern: p, .. } => pattern(p, &mut f),
        Message::Select { selectors, variants, .. } => {
            for s in selectors {
                f(s);
            }
            for v in variants {
                pattern(&v.pattern, &mut f);
            }
        }
    }
}

/// External variables of a message: every referenced name that is not a
/// `.local` (an `.input` name is external).
pub fn external_vars(msg: &Message) -> Vec<String> {
    let locals: Vec<&str> = msg
        .decls()
        .iter()
        .filter_map(|d| match d {
            Decl::Local { name, .. } => Some(name.as_str()),
            Decl::Input { .. } => None,
        })
        .collect();
    let mut out: Vec<String> = Vec::new();
    visit_vars(msg, |v| {
        if !locals.contains(&v) && !out.iter().any(|o| o == v) {
            out.push(v.to_owned());
        }
    });
    out
}

/// Markup names used by a message, in first-use order.
pub fn markup_names(msg: &Message) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: &Pattern| {
        for part in p {
            if let Part::Markup(m) = part
                && !out.contains(&m.name)
            {
                out.push(m.name.clone());
            }
        }
    };
    match msg {
        Message::Pattern { pattern, .. } => add(pattern),
        Message::Select { variants, .. } => variants.iter().for_each(|v| add(&v.pattern)),
    }
    out
}

/// Function names used by a message.
pub fn function_names(msg: &Message) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |e: &Expr| {
        if let Some(f) = &e.function
            && !out.contains(&f.name)
        {
            out.push(f.name.clone());
        }
    };
    for d in msg.decls() {
        match d {
            Decl::Input { expr, .. } | Decl::Local { expr, .. } => add(expr),
        }
    }
    let mut pat = |p: &Pattern| {
        for part in p {
            if let Part::Expr(e) = part {
                add(e);
            }
        }
    };
    match msg {
        Message::Pattern { pattern, .. } => pat(pattern),
        Message::Select { variants, .. } => variants.iter().for_each(|v| pat(&v.pattern)),
    }
    out
}

/// Total text bytes of the text parts (for reporting).
pub fn text_bytes(msg: &Message) -> usize {
    let pat = |p: &Pattern| {
        p.iter()
            .map(|part| match part {
                Part::Text(t) => t.len(),
                _ => 0,
            })
            .sum::<usize>()
    };
    match msg {
        Message::Pattern { pattern, .. } => pat(pattern),
        Message::Select { variants, .. } => variants.iter().map(|v| pat(&v.pattern)).sum(),
    }
}
