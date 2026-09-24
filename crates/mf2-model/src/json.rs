//! Feature `serde`: (de)serialization as the JSON of
//! `spec/data-model/message.json`.
//!
//! * Objects carry their `type` tag (`"message"`, `"select"`, `"input"`,
//!   `"local"`, `"expression"`, `"markup"`, `"function"`, `"literal"`,
//!   `"variable"`, `"*"`); variants have none.
//! * A catch-all key is `{"type": "*"}` (plus `"value"` if it has one); an
//!   attribute without a value is `true`.
//! * Serialization writes `declarations`, `options` and `attributes` even when
//!   empty. A model with two options (or two attributes) of the same name
//!   cannot be serialized: a JSON object cannot hold both.
//! * Deserialization accepts the schema's relaxations (missing empty
//!   `declarations`, `options`, `attributes`), ignores unknown fields (data
//!   model, "Model Extensions"), borrows strings from the input where the
//!   deserializer allows it, and normalizes patterns as [`Pattern::push`]
//!   does (empty text dropped, adjacent text merged).

use alloc::borrow::Cow;
use alloc::vec::Vec;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, SerializeMap, SerializeSeq, Serializer};

use crate::expression::{
    Attributes, Expression, FunctionExpression, FunctionRef, Literal, LiteralExpression, Markup,
    MarkupKind, OptionValue, Options, VariableExpression, VariableRef,
};
use crate::message::{
    CatchAllKey, Declaration, InputDeclaration, Key, LocalDeclaration, Message, PatternMessage,
    SelectMessage, Variant,
};
use crate::pattern::{Pattern, PatternPart};

// ───────────────────────────── serialization ─────────────────────────────

impl Serialize for Message<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Message::Pattern(m) => m.serialize(s),
            Message::Select(m) => m.serialize(s),
        }
    }
}

impl Serialize for PatternMessage<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("type", "message")?;
        map.serialize_entry("declarations", &self.declarations)?;
        map.serialize_entry("pattern", &self.pattern)?;
        map.end()
    }
}

impl Serialize for SelectMessage<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(4))?;
        map.serialize_entry("type", "select")?;
        map.serialize_entry("declarations", &self.declarations)?;
        map.serialize_entry("selectors", &self.selectors)?;
        map.serialize_entry("variants", &self.variants)?;
        map.end()
    }
}

impl Serialize for Declaration<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Declaration::Input(d) => d.serialize(s),
            Declaration::Local(d) => d.serialize(s),
        }
    }
}

impl Serialize for InputDeclaration<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("type", "input")?;
        map.serialize_entry("name", &*self.name)?;
        map.serialize_entry("value", &self.value)?;
        map.end()
    }
}

impl Serialize for LocalDeclaration<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("type", "local")?;
        map.serialize_entry("name", &*self.name)?;
        map.serialize_entry("value", &self.value)?;
        map.end()
    }
}

impl Serialize for Variant<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("keys", &self.keys)?;
        map.serialize_entry("value", &self.value)?;
        map.end()
    }
}

impl Serialize for Key<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Key::Literal(l) => l.serialize(s),
            Key::CatchAll(c) => c.serialize(s),
        }
    }
}

impl Serialize for CatchAllKey<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(1 + usize::from(self.value.is_some())))?;
        map.serialize_entry("type", "*")?;
        if let Some(v) = &self.value {
            map.serialize_entry("value", &**v)?;
        }
        map.end()
    }
}

impl Serialize for Pattern<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.len()))?;
        for part in self.parts() {
            seq.serialize_element(part)?;
        }
        seq.end()
    }
}

impl Serialize for PatternPart<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PatternPart::Text(t) => s.serialize_str(t),
            PatternPart::Expression(e) => e.serialize(s),
            PatternPart::Markup(m) => m.serialize(s),
        }
    }
}

#[derive(Clone, Copy)]
enum ArgRef<'r, 'a> {
    Literal(&'r Literal<'a>),
    Variable(&'r VariableRef<'a>),
}

fn expression_map<S: Serializer>(
    s: S,
    arg: Option<ArgRef<'_, '_>>,
    function: Option<&FunctionRef<'_>>,
    attributes: &Attributes<'_>,
) -> Result<S::Ok, S::Error> {
    let len = 2 + usize::from(arg.is_some()) + usize::from(function.is_some());
    let mut map = s.serialize_map(Some(len))?;
    map.serialize_entry("type", "expression")?;
    match arg {
        Some(ArgRef::Literal(l)) => map.serialize_entry("arg", l)?,
        Some(ArgRef::Variable(v)) => map.serialize_entry("arg", v)?,
        None => {}
    }
    if let Some(f) = function {
        map.serialize_entry("function", f)?;
    }
    map.serialize_entry("attributes", attributes)?;
    map.end()
}

impl Serialize for Expression<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Expression::Literal(e) => e.serialize(s),
            Expression::Variable(e) => e.serialize(s),
            Expression::Function(e) => e.serialize(s),
        }
    }
}

impl Serialize for LiteralExpression<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        expression_map(
            s,
            Some(ArgRef::Literal(&self.arg)),
            self.function.as_ref(),
            &self.attributes,
        )
    }
}

impl Serialize for VariableExpression<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        expression_map(
            s,
            Some(ArgRef::Variable(&self.arg)),
            self.function.as_ref(),
            &self.attributes,
        )
    }
}

impl Serialize for FunctionExpression<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        expression_map(s, None, Some(&self.function), &self.attributes)
    }
}

impl Serialize for Literal<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("type", "literal")?;
        map.serialize_entry("value", &*self.value)?;
        map.end()
    }
}

impl Serialize for VariableRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("type", "variable")?;
        map.serialize_entry("name", &*self.name)?;
        map.end()
    }
}

impl Serialize for FunctionRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("type", "function")?;
        map.serialize_entry("name", &*self.name)?;
        map.serialize_entry("options", &self.options)?;
        map.end()
    }
}

impl Serialize for OptionValue<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            OptionValue::Literal(l) => l.serialize(s),
            OptionValue::Variable(v) => v.serialize(s),
        }
    }
}

/// `true` if two names in `names` are equal (bytewise): pairwise for a few,
/// sorted beyond that, so a large map cannot make serialization quadratic.
fn has_duplicate<'n, I: Iterator<Item = &'n str> + Clone>(names: &I) -> bool {
    if names.clone().nth(16).is_none() {
        return names
            .clone()
            .enumerate()
            .any(|(i, a)| names.clone().skip(i + 1).any(|b| a == b));
    }
    let mut sorted: Vec<&str> = names.clone().collect();
    sorted.sort_unstable();
    sorted.windows(2).any(|w| w[0] == w[1])
}

impl Serialize for Options<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if has_duplicate(&self.0.iter().map(|(k, _)| k.as_ref())) {
            return Err(ser::Error::custom(
                "duplicate option name: a JSON object cannot hold both options",
            ));
        }
        let mut map = s.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl Serialize for Attributes<'_> {
    /// A repeated name writes only its last occurrence: the syntax allows
    /// the repeat (unique names are only a SHOULD) and "all but the last
    /// attribute with the same identifier are ignored" (`syntax.md`,
    /// "Attributes"), while a JSON object holds a name once. The model keeps
    /// every occurrence, as written.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let names = self.0.iter().map(|(k, _)| k.as_ref());
        let last = |i: usize, k: &str| !self.0[i + 1..].iter().any(|(j, _)| j.as_ref() == k);
        let kept: Vec<usize> = if has_duplicate(&names) {
            (0..self.len())
                .filter(|&i| last(i, self.0[i].0.as_ref()))
                .collect()
        } else {
            (0..self.len()).collect()
        };
        let mut map = s.serialize_map(Some(kept.len()))?;
        for i in kept {
            let (k, v) = &self.0[i];
            match v {
                Some(l) => map.serialize_entry(k, l)?,
                None => map.serialize_entry(k, &true)?,
            }
        }
        map.end()
    }
}

impl Serialize for Markup<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(5))?;
        map.serialize_entry("type", "markup")?;
        map.serialize_entry("kind", self.kind.as_json())?;
        map.serialize_entry("name", &*self.name)?;
        map.serialize_entry("options", &self.options)?;
        map.serialize_entry("attributes", &self.attributes)?;
        map.end()
    }
}

impl MarkupKind {
    fn as_json(self) -> &'static str {
        match self {
            MarkupKind::Open => "open",
            MarkupKind::Standalone => "standalone",
            MarkupKind::Close => "close",
        }
    }
}

// ──────────────────────────── deserialization ────────────────────────────

/// A string borrowed from the input when the deserializer allows it.
struct CowStr<'de>(Cow<'de, str>);

impl<'de> Deserialize<'de> for CowStr<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = CowStr<'de>;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a string")
            }
            fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(CowStr(Cow::Borrowed(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(CowStr(Cow::Owned(v.into())))
            }
            fn visit_string<E: de::Error>(
                self,
                v: alloc::string::String,
            ) -> Result<Self::Value, E> {
                Ok(CowStr(Cow::Owned(v)))
            }
        }
        d.deserialize_str(V)
    }
}

/// A JSON value, strings borrowed where possible, object members in order
/// (duplicates kept).
enum Value<'de> {
    Null,
    Bool(bool),
    Number,
    Str(Cow<'de, str>),
    Seq(Vec<Value<'de>>),
    Map(Vec<(Cow<'de, str>, Value<'de>)>),
}

impl<'de> Deserialize<'de> for Value<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Value<'de>;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Value::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Value::Number)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Value::Number)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Value::Number)
            }
            fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(Value::Str(Cow::Borrowed(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Value::Str(Cow::Owned(v.into())))
            }
            fn visit_string<E: de::Error>(
                self,
                v: alloc::string::String,
            ) -> Result<Self::Value, E> {
                Ok(Value::Str(Cow::Owned(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }
            fn visit_some<D2: Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
                Value::deserialize(d)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(64));
                while let Some(v) = seq.next_element()? {
                    out.push(v);
                }
                Ok(Value::Seq(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::with_capacity(map.size_hint().unwrap_or(0).min(64));
                while let Some(CowStr(k)) = map.next_key()? {
                    out.push((k, map.next_value()?));
                }
                Ok(Value::Map(out))
            }
        }
        d.deserialize_any(V)
    }
}

type Members<'de> = Vec<(Cow<'de, str>, Value<'de>)>;

/// The members of an object, with one-shot field extraction.
struct Obj<'de>(Members<'de>);

impl<'de> Obj<'de> {
    fn new<E: de::Error>(v: Value<'de>, what: &'static str) -> Result<Self, E> {
        match v {
            Value::Map(m) => Ok(Obj(m)),
            _ => Err(E::custom(what)),
        }
    }

    /// Removes and returns the first member named `name`.
    fn take(&mut self, name: &str) -> Option<Value<'de>> {
        let i = self.0.iter().position(|(k, _)| k == name)?;
        Some(self.0.swap_remove(i).1)
    }

    fn string<E: de::Error>(&mut self, name: &str, what: &'static str) -> Result<Cow<'de, str>, E> {
        match self.take(name) {
            Some(Value::Str(s)) => Ok(s),
            _ => Err(E::custom(what)),
        }
    }

    /// The `type` tag.
    fn tag<E: de::Error>(&mut self) -> Result<Cow<'de, str>, E> {
        self.string("type", "an object needs a string `type`")
    }

    fn expect_tag<E: de::Error>(&mut self, tag: &str, what: &'static str) -> Result<(), E> {
        if self.tag::<E>()? == tag {
            Ok(())
        } else {
            Err(E::custom(what))
        }
    }

    /// An array member that may be missing (the schema's relaxation).
    fn seq_or_empty<E: de::Error>(
        &mut self,
        name: &str,
        what: &'static str,
    ) -> Result<Vec<Value<'de>>, E> {
        match self.take(name) {
            None => Ok(Vec::new()),
            Some(Value::Seq(v)) => Ok(v),
            Some(_) => Err(E::custom(what)),
        }
    }

    fn seq<E: de::Error>(&mut self, name: &str, what: &'static str) -> Result<Vec<Value<'de>>, E> {
        match self.take(name) {
            Some(Value::Seq(v)) => Ok(v),
            _ => Err(E::custom(what)),
        }
    }

    /// An object member that may be missing (the schema's relaxation).
    fn map_or_empty<E: de::Error>(
        &mut self,
        name: &str,
        what: &'static str,
    ) -> Result<Members<'de>, E> {
        match self.take(name) {
            None => Ok(Vec::new()),
            Some(Value::Map(m)) => Ok(m),
            Some(_) => Err(E::custom(what)),
        }
    }
}

fn message<E: de::Error>(v: Value<'_>) -> Result<Message<'_>, E> {
    let mut o = Obj::new(v, "a message must be an object")?;
    let tag = o.tag::<E>()?;
    let declarations = o
        .seq_or_empty::<E>("declarations", "`declarations` must be an array")?
        .into_iter()
        .map(declaration)
        .collect::<Result<Vec<_>, E>>()?;
    match &*tag {
        "message" => Ok(Message::Pattern(PatternMessage {
            declarations,
            pattern: pattern(
                o.take("pattern")
                    .ok_or_else(|| E::custom("a message needs a `pattern`"))?,
            )?,
        })),
        "select" => {
            let selectors = o
                .seq::<E>("selectors", "a select message needs a `selectors` array")?
                .into_iter()
                .map(variable_ref)
                .collect::<Result<Vec<_>, E>>()?;
            let variants = o
                .seq::<E>("variants", "a select message needs a `variants` array")?
                .into_iter()
                .map(variant)
                .collect::<Result<Vec<_>, E>>()?;
            Ok(Message::Select(SelectMessage {
                declarations,
                selectors,
                variants,
            }))
        }
        _ => Err(E::custom(
            "a message's `type` must be \"message\" or \"select\"",
        )),
    }
}

fn declaration<E: de::Error>(v: Value<'_>) -> Result<Declaration<'_>, E> {
    let mut o = Obj::new(v, "a declaration must be an object")?;
    let tag = o.tag::<E>()?;
    let name = o.string::<E>("name", "a declaration needs a string `name`")?;
    let value = o
        .take("value")
        .ok_or_else(|| E::custom("a declaration needs a `value`"))?;
    match &*tag {
        "input" => {
            let Expression::Variable(value) = expression(value)? else {
                return Err(E::custom(
                    "an input declaration's value needs a variable `arg`",
                ));
            };
            if value.arg.name != name {
                return Err(E::custom(
                    "an input declaration's `name` must equal its variable's name",
                ));
            }
            Ok(Declaration::Input(InputDeclaration { name, value }))
        }
        "local" => Ok(Declaration::Local(LocalDeclaration {
            name,
            value: expression(value)?,
        })),
        _ => Err(E::custom(
            "a declaration's `type` must be \"input\" or \"local\"",
        )),
    }
}

fn variant<E: de::Error>(v: Value<'_>) -> Result<Variant<'_>, E> {
    let mut o = Obj::new(v, "a variant must be an object")?;
    let keys = o
        .seq::<E>("keys", "a variant needs a `keys` array")?
        .into_iter()
        .map(key)
        .collect::<Result<Vec<_>, E>>()?;
    let value = pattern(
        o.take("value")
            .ok_or_else(|| E::custom("a variant needs a `value` pattern"))?,
    )?;
    Ok(Variant { keys, value })
}

fn key<E: de::Error>(v: Value<'_>) -> Result<Key<'_>, E> {
    let mut o = Obj::new(v, "a key must be an object")?;
    match &*o.tag::<E>()? {
        "literal" => Ok(Key::Literal(Literal {
            value: o.string::<E>("value", "a literal needs a string `value`")?,
        })),
        "*" => {
            let value = match o.take("value") {
                None => None,
                Some(Value::Str(s)) => Some(s),
                Some(_) => return Err(E::custom("a catch-all key's `value` must be a string")),
            };
            Ok(Key::CatchAll(CatchAllKey { value }))
        }
        _ => Err(E::custom("a key's `type` must be \"literal\" or \"*\"")),
    }
}

fn pattern<E: de::Error>(v: Value<'_>) -> Result<Pattern<'_>, E> {
    let Value::Seq(items) = v else {
        return Err(E::custom("a pattern must be an array"));
    };
    let mut p = Pattern::with_capacity(items.len());
    for item in items {
        let part = match item {
            Value::Str(s) => PatternPart::Text(s),
            Value::Map(m) => {
                let is_markup = m
                    .iter()
                    .any(|(k, v)| k == "type" && matches!(v, Value::Str(t) if t == "markup"));
                if is_markup {
                    PatternPart::Markup(markup(Value::Map(m))?)
                } else {
                    PatternPart::Expression(expression(Value::Map(m))?)
                }
            }
            _ => return Err(E::custom("a pattern element must be a string or an object")),
        };
        p.push(part);
    }
    Ok(p)
}

enum Arg<'de> {
    Literal(Literal<'de>),
    Variable(VariableRef<'de>),
}

fn arg<E: de::Error>(v: Value<'_>) -> Result<Arg<'_>, E> {
    let mut o = Obj::new(v, "a literal or variable must be an object")?;
    match &*o.tag::<E>()? {
        "literal" => Ok(Arg::Literal(Literal {
            value: o.string::<E>("value", "a literal needs a string `value`")?,
        })),
        "variable" => Ok(Arg::Variable(VariableRef {
            name: o.string::<E>("name", "a variable needs a string `name`")?,
        })),
        _ => Err(E::custom("expected `type` \"literal\" or \"variable\"")),
    }
}

fn variable_ref<E: de::Error>(v: Value<'_>) -> Result<VariableRef<'_>, E> {
    match arg(v)? {
        Arg::Variable(v) => Ok(v),
        Arg::Literal(_) => Err(E::custom("a selector must be a variable")),
    }
}

fn literal<E: de::Error>(v: Value<'_>) -> Result<Literal<'_>, E> {
    match arg(v)? {
        Arg::Literal(l) => Ok(l),
        Arg::Variable(_) => Err(E::custom("expected a literal")),
    }
}

fn expression<E: de::Error>(v: Value<'_>) -> Result<Expression<'_>, E> {
    let mut o = Obj::new(v, "an expression must be an object")?;
    o.expect_tag::<E>(
        "expression",
        "an expression's `type` must be \"expression\"",
    )?;
    let arg = o.take("arg").map(arg::<E>).transpose()?;
    let function = o.take("function").map(function_ref::<E>).transpose()?;
    let attributes =
        attributes::<E>(o.map_or_empty::<E>("attributes", "`attributes` must be an object")?)?;
    Ok(match (arg, function) {
        (Some(Arg::Literal(arg)), function) => Expression::Literal(LiteralExpression {
            arg,
            function,
            attributes,
        }),
        (Some(Arg::Variable(arg)), function) => Expression::Variable(VariableExpression {
            arg,
            function,
            attributes,
        }),
        (None, Some(function)) => Expression::Function(FunctionExpression {
            function,
            attributes,
        }),
        (None, None) => return Err(E::custom("an expression needs an `arg` or a `function`")),
    })
}

fn function_ref<E: de::Error>(v: Value<'_>) -> Result<FunctionRef<'_>, E> {
    let mut o = Obj::new(v, "a function must be an object")?;
    o.expect_tag::<E>("function", "a function's `type` must be \"function\"")?;
    let name = o.string::<E>("name", "a function needs a string `name`")?;
    let options = options::<E>(o.map_or_empty::<E>("options", "`options` must be an object")?)?;
    Ok(FunctionRef { name, options })
}

fn options<E: de::Error>(members: Members<'_>) -> Result<Options<'_>, E> {
    let mut out = Options::with_capacity(members.len());
    for (k, v) in members {
        let value = match arg(v)? {
            Arg::Literal(l) => OptionValue::Literal(l),
            Arg::Variable(v) => OptionValue::Variable(v),
        };
        out.push(k, value);
    }
    Ok(out)
}

fn attributes<E: de::Error>(members: Members<'_>) -> Result<Attributes<'_>, E> {
    let mut out = Attributes::with_capacity(members.len());
    for (k, v) in members {
        let value = match v {
            Value::Bool(true) => None,
            Value::Map(_) => Some(literal(v)?),
            _ => {
                return Err(E::custom(
                    "an attribute's value must be a literal or `true`",
                ));
            }
        };
        out.push(k, value);
    }
    Ok(out)
}

fn markup<E: de::Error>(v: Value<'_>) -> Result<Markup<'_>, E> {
    let mut o = Obj::new(v, "markup must be an object")?;
    o.expect_tag::<E>("markup", "markup's `type` must be \"markup\"")?;
    let kind = match &*o.string::<E>("kind", "markup needs a string `kind`")? {
        "open" => MarkupKind::Open,
        "standalone" => MarkupKind::Standalone,
        "close" => MarkupKind::Close,
        _ => {
            return Err(E::custom(
                "markup's `kind` must be open, standalone or close",
            ));
        }
    };
    let name = o.string::<E>("name", "markup needs a string `name`")?;
    let options = options::<E>(o.map_or_empty::<E>("options", "`options` must be an object")?)?;
    let attributes =
        attributes::<E>(o.map_or_empty::<E>("attributes", "`attributes` must be an object")?)?;
    Ok(Markup {
        kind,
        name,
        options,
        attributes,
    })
}

macro_rules! deserialize_via {
    ($ty:ident, $f:ident) => {
        impl<'de: 'a, 'a> Deserialize<'de> for $ty<'a> {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $f(Value::deserialize(d)?)
            }
        }
    };
}

deserialize_via!(Message, message);
deserialize_via!(Declaration, declaration);
deserialize_via!(Variant, variant);
deserialize_via!(Key, key);
deserialize_via!(Pattern, pattern);
deserialize_via!(Expression, expression);
deserialize_via!(FunctionRef, function_ref);
deserialize_via!(Markup, markup);
deserialize_via!(Literal, literal);
deserialize_via!(VariableRef, variable_ref);
