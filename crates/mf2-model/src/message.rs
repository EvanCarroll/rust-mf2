//! Messages, declarations and variants (`spec/data-model/README.md`,
//! "Message Model").

use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::expression::{Expression, Literal, VariableExpression, VariableRef};
use crate::pattern::Pattern;

/// A message: a single pattern, or a selection among variants.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
// Future versions of MF2 may define new structures (the specification's
// stability policy); a new one is a new variant, which a 1.x minor may add.
#[non_exhaustive]
pub enum Message<'a> {
    /// No selectors: one pattern.
    Pattern(PatternMessage<'a>),
    /// `.match` with selectors and variants.
    Select(SelectMessage<'a>),
}

impl<'a> Message<'a> {
    /// The declarations of either kind of message.
    pub fn declarations(&self) -> &[Declaration<'a>] {
        match self {
            Message::Pattern(m) => &m.declarations,
            Message::Select(m) => &m.declarations,
        }
    }

    /// The same message with every string owned.
    pub fn into_owned(self) -> Message<'static> {
        match self {
            Message::Pattern(m) => Message::Pattern(m.into_owned()),
            Message::Select(m) => Message::Select(m.into_owned()),
        }
    }
}

/// A message without selectors: declarations and one pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PatternMessage<'a> {
    /// `.input` and `.local` declarations, in source order.
    pub declarations: Vec<Declaration<'a>>,
    /// The pattern.
    pub pattern: Pattern<'a>,
}

impl PatternMessage<'_> {
    /// The same message with every string owned.
    pub fn into_owned(self) -> PatternMessage<'static> {
        PatternMessage {
            declarations: self
                .declarations
                .into_iter()
                .map(Declaration::into_owned)
                .collect(),
            pattern: self.pattern.into_owned(),
        }
    }
}

/// A message with a matcher: declarations, selectors and variants.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SelectMessage<'a> {
    /// `.input` and `.local` declarations, in source order.
    pub declarations: Vec<Declaration<'a>>,
    /// The selectors, in source order.
    pub selectors: Vec<VariableRef<'a>>,
    /// The variants, in source order.
    pub variants: Vec<Variant<'a>>,
}

impl SelectMessage<'_> {
    /// The same message with every string owned.
    pub fn into_owned(self) -> SelectMessage<'static> {
        SelectMessage {
            declarations: self
                .declarations
                .into_iter()
                .map(Declaration::into_owned)
                .collect(),
            selectors: self
                .selectors
                .into_iter()
                .map(VariableRef::into_owned)
                .collect(),
            variants: self.variants.into_iter().map(Variant::into_owned).collect(),
        }
    }
}

/// A declaration.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
// Future versions of MF2 may define new structures (the specification's
// stability policy); a new one is a new variant, which a 1.x minor may add.
#[non_exhaustive]
pub enum Declaration<'a> {
    /// `.input {$name …}`.
    Input(InputDeclaration<'a>),
    /// `.local $name = {…}`.
    Local(LocalDeclaration<'a>),
}

impl Declaration<'_> {
    /// The variable the declaration binds.
    pub fn name(&self) -> &str {
        match self {
            Declaration::Input(d) => &d.name,
            Declaration::Local(d) => &d.name,
        }
    }

    /// The same declaration with every string owned.
    pub fn into_owned(self) -> Declaration<'static> {
        match self {
            Declaration::Input(d) => Declaration::Input(d.into_owned()),
            Declaration::Local(d) => Declaration::Local(d.into_owned()),
        }
    }
}

/// `.input {$name …}`: binds an external variable, optionally through a
/// function.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct InputDeclaration<'a> {
    /// The bound variable; equals `value.arg.name`.
    pub name: Cow<'a, str>,
    /// The variable expression.
    pub value: VariableExpression<'a>,
}

impl InputDeclaration<'_> {
    /// The same declaration with every string owned.
    pub fn into_owned(self) -> InputDeclaration<'static> {
        InputDeclaration {
            name: Cow::Owned(self.name.into_owned()),
            value: self.value.into_owned(),
        }
    }
}

/// `.local $name = {…}`: binds a local variable to an expression.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LocalDeclaration<'a> {
    /// The bound variable.
    pub name: Cow<'a, str>,
    /// The expression.
    pub value: Expression<'a>,
}

impl LocalDeclaration<'_> {
    /// The same declaration with every string owned.
    pub fn into_owned(self) -> LocalDeclaration<'static> {
        LocalDeclaration {
            name: Cow::Owned(self.name.into_owned()),
            value: self.value.into_owned(),
        }
    }
}

/// A variant: one key per selector, and a pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Variant<'a> {
    /// The keys, in source order.
    pub keys: Vec<Key<'a>>,
    /// The pattern.
    pub value: Pattern<'a>,
}

impl Variant<'_> {
    /// The same variant with every string owned.
    pub fn into_owned(self) -> Variant<'static> {
        Variant {
            keys: self.keys.into_iter().map(Key::into_owned).collect(),
            value: self.value.into_owned(),
        }
    }
}

/// A variant key.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
// Future versions of MF2 may define new structures (the specification's
// stability policy); a new one is a new variant, which a 1.x minor may add.
#[non_exhaustive]
pub enum Key<'a> {
    /// A literal key.
    Literal(Literal<'a>),
    /// The catch-all key `*`.
    CatchAll(CatchAllKey<'a>),
}

impl Key<'_> {
    /// The same key with every string owned.
    pub fn into_owned(self) -> Key<'static> {
        match self {
            Key::Literal(l) => Key::Literal(l.into_owned()),
            Key::CatchAll(c) => Key::CatchAll(c.into_owned()),
        }
    }
}

/// The catch-all key. Its `value` lets other formats keep an identifier; it
/// is always `None` from MF2 syntax, where the key is written `*`.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct CatchAllKey<'a> {
    /// An identifier kept from another format, if any.
    pub value: Option<Cow<'a, str>>,
}

impl CatchAllKey<'_> {
    /// The same key with every string owned.
    pub fn into_owned(self) -> CatchAllKey<'static> {
        CatchAllKey {
            value: self.value.map(|v| Cow::Owned(v.into_owned())),
        }
    }
}
