//! Expressions, literals, variables, functions, options, markup and
//! attributes (`spec/data-model/README.md`, "Pattern Model" to "Attribute
//! Model").

use alloc::borrow::Cow;
use alloc::vec::Vec;

/// An expression: an operand with an optional function, or a function alone.
///
/// Not exhaustive: the spec says future versions may add expression shapes.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum Expression<'a> {
    /// `{|literal| :fn …}` or `{literal}`.
    Literal(LiteralExpression<'a>),
    /// `{$var :fn …}` or `{$var}`.
    Variable(VariableExpression<'a>),
    /// `{:fn …}`.
    Function(FunctionExpression<'a>),
}

/// An expression whose operand is a literal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LiteralExpression<'a> {
    /// The operand.
    pub arg: Literal<'a>,
    /// The function applied to it, if any.
    pub function: Option<FunctionRef<'a>>,
    /// The expression's attributes.
    pub attributes: Attributes<'a>,
}

/// An expression whose operand is a variable.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct VariableExpression<'a> {
    /// The operand.
    pub arg: VariableRef<'a>,
    /// The function applied to it, if any.
    pub function: Option<FunctionRef<'a>>,
    /// The expression's attributes.
    pub attributes: Attributes<'a>,
}

/// An expression with a function and no operand.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FunctionExpression<'a> {
    /// The function.
    pub function: FunctionRef<'a>,
    /// The expression's attributes.
    pub attributes: Attributes<'a>,
}

impl<'a> Expression<'a> {
    /// The function, if the expression has one.
    pub fn function(&self) -> Option<&FunctionRef<'a>> {
        match self {
            Expression::Literal(e) => e.function.as_ref(),
            Expression::Variable(e) => e.function.as_ref(),
            Expression::Function(e) => Some(&e.function),
        }
    }

    /// The expression's attributes.
    pub fn attributes(&self) -> &Attributes<'a> {
        match self {
            Expression::Literal(e) => &e.attributes,
            Expression::Variable(e) => &e.attributes,
            Expression::Function(e) => &e.attributes,
        }
    }

    /// The same expression with every string owned.
    pub fn into_owned(self) -> Expression<'static> {
        match self {
            Expression::Literal(e) => Expression::Literal(e.into_owned()),
            Expression::Variable(e) => Expression::Variable(e.into_owned()),
            Expression::Function(e) => Expression::Function(e.into_owned()),
        }
    }
}

impl LiteralExpression<'_> {
    /// The same expression with every string owned.
    pub fn into_owned(self) -> LiteralExpression<'static> {
        LiteralExpression {
            arg: self.arg.into_owned(),
            function: self.function.map(FunctionRef::into_owned),
            attributes: self.attributes.into_owned(),
        }
    }
}

impl VariableExpression<'_> {
    /// The same expression with every string owned.
    pub fn into_owned(self) -> VariableExpression<'static> {
        VariableExpression {
            arg: self.arg.into_owned(),
            function: self.function.map(FunctionRef::into_owned),
            attributes: self.attributes.into_owned(),
        }
    }
}

impl FunctionExpression<'_> {
    /// The same expression with every string owned.
    pub fn into_owned(self) -> FunctionExpression<'static> {
        FunctionExpression {
            function: self.function.into_owned(),
            attributes: self.attributes.into_owned(),
        }
    }
}

fn own(s: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(s.into_owned())
}

/// A literal: its **cooked** value (escapes processed). Whether it was
/// quoted is not data-model information and is not kept.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Literal<'a> {
    /// The string value.
    pub value: Cow<'a, str>,
}

impl Literal<'_> {
    /// The same literal, owned.
    pub fn into_owned(self) -> Literal<'static> {
        Literal {
            value: own(self.value),
        }
    }
}

/// A variable reference, by name (without the `$` sigil).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct VariableRef<'a> {
    /// The name, as written, without bidi marks.
    pub name: Cow<'a, str>,
}

impl VariableRef<'_> {
    /// The same reference, owned.
    pub fn into_owned(self) -> VariableRef<'static> {
        VariableRef {
            name: own(self.name),
        }
    }
}

/// A function reference: its identifier and options.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FunctionRef<'a> {
    /// The identifier, e.g. `"number"` or `"ns:fn"`, without the `:` sigil.
    pub name: Cow<'a, str>,
    /// The options, in source order.
    pub options: Options<'a>,
}

impl FunctionRef<'_> {
    /// The same reference, owned.
    pub fn into_owned(self) -> FunctionRef<'static> {
        FunctionRef {
            name: own(self.name),
            options: self.options.into_owned(),
        }
    }
}

/// An option's value: a literal or a variable.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum OptionValue<'a> {
    /// `opt=|literal|` or `opt=literal`.
    Literal(Literal<'a>),
    /// `opt=$var`.
    Variable(VariableRef<'a>),
}

impl OptionValue<'_> {
    /// The same value, owned.
    pub fn into_owned(self) -> OptionValue<'static> {
        match self {
            OptionValue::Literal(l) => OptionValue::Literal(l.into_owned()),
            OptionValue::Variable(v) => OptionValue::Variable(v.into_owned()),
        }
    }
}

/// The options of a function or markup, in source order.
///
/// Duplicates are representable, so that validation can report *Duplicate
/// Option Name*; a valid message has none.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Options<'a>(pub(crate) Vec<(Cow<'a, str>, OptionValue<'a>)>);

impl<'a> Options<'a> {
    /// No options (does not allocate).
    pub const fn new() -> Self {
        Options(Vec::new())
    }

    /// No options, with room for `capacity` without reallocating.
    pub fn with_capacity(capacity: usize) -> Self {
        Options(Vec::with_capacity(capacity))
    }

    /// Appends an option (duplicates are kept).
    pub fn push(&mut self, name: Cow<'a, str>, value: OptionValue<'a>) {
        self.0.push((name, value));
    }

    /// `(identifier, value)` pairs, in source order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue<'a>)> {
        self.0.iter().map(|(k, v)| (k.as_ref(), v))
    }

    /// The value of the first option named exactly `name`.
    pub fn get(&self, name: &str) -> Option<&OptionValue<'a>> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// How many options (duplicates included).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// No options.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The same options, owned.
    pub fn into_owned(self) -> Options<'static> {
        Options(
            self.0
                .into_iter()
                .map(|(k, v)| (own(k), v.into_owned()))
                .collect(),
        )
    }
}

impl<'a> FromIterator<(Cow<'a, str>, OptionValue<'a>)> for Options<'a> {
    fn from_iter<I: IntoIterator<Item = (Cow<'a, str>, OptionValue<'a>)>>(iter: I) -> Self {
        Options(iter.into_iter().collect())
    }
}

/// Markup: `{#name …}`, `{#name … /}` or `{/name …}`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Markup<'a> {
    /// Open, standalone or close.
    pub kind: MarkupKind,
    /// The identifier, without the `#` / `/` sigils.
    pub name: Cow<'a, str>,
    /// The options, in source order.
    pub options: Options<'a>,
    /// The attributes, in source order.
    pub attributes: Attributes<'a>,
}

impl Markup<'_> {
    /// The same markup, owned.
    pub fn into_owned(self) -> Markup<'static> {
        Markup {
            kind: self.kind,
            name: own(self.name),
            options: self.options.into_owned(),
            attributes: self.attributes.into_owned(),
        }
    }
}

/// The three forms of markup.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MarkupKind {
    /// `{#name}`.
    Open,
    /// `{#name /}`.
    Standalone,
    /// `{/name}`.
    Close,
}

/// The attributes of an expression or markup, in source order; a `None`
/// value is the spec's `true` (an attribute written without a value).
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Attributes<'a>(pub(crate) Vec<(Cow<'a, str>, Option<Literal<'a>>)>);

impl<'a> Attributes<'a> {
    /// No attributes (does not allocate).
    pub const fn new() -> Self {
        Attributes(Vec::new())
    }

    /// No attributes, with room for `capacity` without reallocating.
    pub fn with_capacity(capacity: usize) -> Self {
        Attributes(Vec::with_capacity(capacity))
    }

    /// Appends an attribute (duplicates are kept).
    pub fn push(&mut self, name: Cow<'a, str>, value: Option<Literal<'a>>) {
        self.0.push((name, value));
    }

    /// `(identifier, value)` pairs, in source order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<&Literal<'a>>)> {
        self.0.iter().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }

    /// The value of the first attribute named exactly `name`: `None` if there
    /// is none, `Some(None)` if it has no value.
    pub fn get(&self, name: &str) -> Option<Option<&Literal<'a>>> {
        self.0
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_ref())
    }

    /// How many attributes (duplicates included).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// No attributes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The same attributes, owned.
    pub fn into_owned(self) -> Attributes<'static> {
        Attributes(
            self.0
                .into_iter()
                .map(|(k, v)| (own(k), v.map(Literal::into_owned)))
                .collect(),
        )
    }
}

impl<'a> FromIterator<(Cow<'a, str>, Option<Literal<'a>>)> for Attributes<'a> {
    fn from_iter<I: IntoIterator<Item = (Cow<'a, str>, Option<Literal<'a>>)>>(iter: I) -> Self {
        Attributes(iter.into_iter().collect())
    }
}

/// Splits an identifier at its namespace separator: `"ns:name"` →
/// `(Some("ns"), "name")`, `"name"` → `(None, "name")`. A name cannot contain
/// `:`, so the first colon is the separator.
pub fn split_identifier(identifier: &str) -> (Option<&str>, &str) {
    match identifier.split_once(':') {
        Some((ns, name)) => (Some(ns), name),
        None => (None, identifier),
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    use super::{
        Attributes, Expression, FunctionExpression, FunctionRef, Literal, OptionValue, Options,
        VariableExpression, VariableRef, split_identifier,
    };

    fn lit(s: &str) -> Literal<'_> {
        Literal {
            value: Cow::Borrowed(s),
        }
    }

    #[test]
    fn options_keep_order_and_duplicates() {
        let mut o = Options::new();
        assert!(o.is_empty());
        o.push("b".into(), OptionValue::Literal(lit("1")));
        o.push(
            "a".into(),
            OptionValue::Variable(VariableRef { name: "x".into() }),
        );
        o.push("b".into(), OptionValue::Literal(lit("2")));
        assert_eq!(o.len(), 3);
        let names: alloc::vec::Vec<&str> = o.iter().map(|(k, _)| k).collect();
        assert_eq!(names, ["b", "a", "b"]);
        // `get` returns the first exact match.
        assert_eq!(o.get("b"), Some(&OptionValue::Literal(lit("1"))));
        assert_eq!(
            o.get("a"),
            Some(&OptionValue::Variable(VariableRef { name: "x".into() }))
        );
        assert_eq!(o.get("c"), None);
        // Exact, not normalized: "é" precomposed vs decomposed differ.
        let mut n = Options::new();
        n.push("\u{e9}".into(), OptionValue::Literal(lit("1")));
        assert!(n.get("e\u{301}").is_none());
    }

    #[test]
    fn attributes_keep_order_and_duplicates() {
        let mut a = Attributes::new();
        a.push("flag".into(), None);
        a.push("x".into(), Some(lit("1")));
        a.push("flag".into(), Some(lit("2")));
        assert_eq!(a.len(), 3);
        assert_eq!(a.get("flag"), Some(None));
        assert_eq!(a.get("x"), Some(Some(&lit("1"))));
        assert_eq!(a.get("y"), None);
        let all: alloc::vec::Vec<(&str, Option<&Literal<'_>>)> = a.iter().collect();
        assert_eq!(all[2], ("flag", Some(&lit("2"))));
    }

    #[test]
    fn split_identifiers() {
        assert_eq!(split_identifier("ns:name"), (Some("ns"), "name"));
        assert_eq!(split_identifier("name"), (None, "name"));
        assert_eq!(split_identifier("u:dir"), (Some("u"), "dir"));
        assert_eq!(split_identifier(""), (None, ""));
    }

    #[test]
    fn expression_accessors_and_owning() {
        let mut attrs = Attributes::new();
        attrs.push("a".into(), None);
        let f = FunctionRef {
            name: "number".into(),
            options: Options::new(),
        };
        let e = Expression::Variable(VariableExpression {
            arg: VariableRef { name: "n".into() },
            function: Some(f.clone()),
            attributes: attrs.clone(),
        });
        assert_eq!(e.function(), Some(&f));
        assert_eq!(e.attributes(), &attrs);
        let fe = Expression::Function(FunctionExpression {
            function: f.clone(),
            attributes: Attributes::new(),
        });
        assert_eq!(fe.function().map(|f| f.name.as_ref()), Some("number"));
        let owned: Expression<'static> = e.clone().into_owned();
        assert_eq!(owned, e);
        if let Expression::Variable(v) = owned {
            assert!(matches!(v.arg.name, Cow::Owned(_)));
        }
    }
}
