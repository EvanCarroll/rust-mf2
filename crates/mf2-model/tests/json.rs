//! Feature `serde`: round trips on hand-written fixtures of every node kind,
//! the schema's relaxations, and what cannot be serialized.

#![cfg(feature = "serde")]

use mf2_model::{
    Attributes, CatchAllKey, Cow, Declaration, Expression, FunctionExpression, FunctionRef,
    InputDeclaration, Key, Literal, LiteralExpression, LocalDeclaration, Markup, MarkupKind,
    Message, OptionValue, Options, Pattern, PatternMessage, PatternPart, SelectMessage,
    VariableExpression, VariableRef, Variant,
};
use serde_json::{Value, json};

fn lit(s: &str) -> Literal<'_> {
    Literal {
        value: Cow::Borrowed(s),
    }
}

fn var(s: &str) -> VariableRef<'_> {
    VariableRef {
        name: Cow::Borrowed(s),
    }
}

fn func<'a>(name: &'a str, options: &[(&'a str, OptionValue<'a>)]) -> FunctionRef<'a> {
    let mut o = Options::new();
    for (k, v) in options {
        o.push(Cow::Borrowed(*k), v.clone());
    }
    FunctionRef {
        name: Cow::Borrowed(name),
        options: o,
    }
}

fn attrs<'a>(items: &[(&'a str, Option<&'a str>)]) -> Attributes<'a> {
    let mut a = Attributes::new();
    for (k, v) in items {
        a.push(Cow::Borrowed(*k), v.map(lit));
    }
    a
}

/// Every node kind of the data model at least once.
fn every_kind() -> Message<'static> {
    let mut pattern = Pattern::new();
    pattern.push(PatternPart::Text("Hi ".into()));
    pattern.push(PatternPart::Expression(Expression::Variable(
        VariableExpression {
            arg: var("user"),
            function: None,
            attributes: attrs(&[("translate", Some("no"))]),
        },
    )));
    pattern.push(PatternPart::Expression(Expression::Literal(
        LiteralExpression {
            arg: lit("a|b"),
            function: Some(func("string", &[])),
            attributes: Attributes::new(),
        },
    )));
    pattern.push(PatternPart::Expression(Expression::Function(
        FunctionExpression {
            function: func(
                "ns:fn",
                &[
                    ("k", OptionValue::Literal(lit("v"))),
                    ("ns:o", OptionValue::Variable(var("x"))),
                ],
            ),
            attributes: attrs(&[("flag", None)]),
        },
    )));
    pattern.push(PatternPart::Markup(Markup {
        kind: MarkupKind::Open,
        name: "b".into(),
        options: func("", &[("id", OptionValue::Literal(lit("1")))]).options,
        attributes: attrs(&[("can-copy", None)]),
    }));
    pattern.push(PatternPart::Markup(Markup {
        kind: MarkupKind::Standalone,
        name: "img".into(),
        options: Options::new(),
        attributes: Attributes::new(),
    }));
    pattern.push(PatternPart::Markup(Markup {
        kind: MarkupKind::Close,
        name: "b".into(),
        options: Options::new(),
        attributes: Attributes::new(),
    }));
    Message::Select(SelectMessage {
        declarations: vec![
            Declaration::Input(InputDeclaration {
                name: "n".into(),
                value: VariableExpression {
                    arg: var("n"),
                    function: Some(func(
                        "number",
                        &[("minimumFractionDigits", OptionValue::Literal(lit("2")))],
                    )),
                    attributes: Attributes::new(),
                },
            }),
            Declaration::Local(LocalDeclaration {
                name: "m".into(),
                value: Expression::Variable(VariableExpression {
                    arg: var("n"),
                    function: None,
                    attributes: Attributes::new(),
                }),
            }),
        ],
        selectors: vec![var("n"), var("m")],
        variants: vec![
            Variant {
                keys: vec![
                    Key::Literal(lit("one")),
                    Key::CatchAll(CatchAllKey::default()),
                ],
                value: pattern,
            },
            Variant {
                keys: vec![
                    Key::CatchAll(CatchAllKey {
                        value: Some("other".into()),
                    }),
                    Key::CatchAll(CatchAllKey::default()),
                ],
                value: Pattern::new(),
            },
        ],
    })
}

fn every_kind_json() -> Value {
    let empty = json!({});
    json!({
        "type": "select",
        "declarations": [
            {"type": "input", "name": "n", "value": {
                "type": "expression",
                "arg": {"type": "variable", "name": "n"},
                "function": {"type": "function", "name": "number",
                    "options": {"minimumFractionDigits": {"type": "literal", "value": "2"}}},
                "attributes": empty}},
            {"type": "local", "name": "m", "value": {
                "type": "expression", "arg": {"type": "variable", "name": "n"}, "attributes": {}}}
        ],
        "selectors": [{"type": "variable", "name": "n"}, {"type": "variable", "name": "m"}],
        "variants": [
            {"keys": [{"type": "literal", "value": "one"}, {"type": "*"}], "value": [
                "Hi ",
                {"type": "expression", "arg": {"type": "variable", "name": "user"},
                    "attributes": {"translate": {"type": "literal", "value": "no"}}},
                {"type": "expression", "arg": {"type": "literal", "value": "a|b"},
                    "function": {"type": "function", "name": "string", "options": {}},
                    "attributes": {}},
                {"type": "expression",
                    "function": {"type": "function", "name": "ns:fn", "options": {
                        "k": {"type": "literal", "value": "v"},
                        "ns:o": {"type": "variable", "name": "x"}}},
                    "attributes": {"flag": true}},
                {"type": "markup", "kind": "open", "name": "b",
                    "options": {"id": {"type": "literal", "value": "1"}},
                    "attributes": {"can-copy": true}},
                {"type": "markup", "kind": "standalone", "name": "img", "options": {}, "attributes": {}},
                {"type": "markup", "kind": "close", "name": "b", "options": {}, "attributes": {}}
            ]},
            {"keys": [{"type": "*", "value": "other"}, {"type": "*"}], "value": []}
        ]
    })
}

#[test]
fn every_node_kind_serializes_to_the_schema_shape() {
    let got = serde_json::to_value(every_kind()).expect("serializable");
    assert_eq!(got, every_kind_json());
}

#[test]
fn every_node_kind_round_trips() {
    let text = serde_json::to_string(&every_kind()).expect("serializable");
    let back: Message<'_> = serde_json::from_str(&text).expect("deserializable");
    assert_eq!(back, every_kind());
}

#[test]
fn pattern_message_round_trips_and_borrows() {
    let m = Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern: Pattern::from_text("Hello".into()),
    });
    let text = serde_json::to_string(&m).expect("serializable");
    assert_eq!(
        text,
        r#"{"type":"message","declarations":[],"pattern":["Hello"]}"#
    );
    let back: Message<'_> = serde_json::from_str(&text).expect("deserializable");
    assert_eq!(back, m);
    let Message::Pattern(p) = back else {
        panic!("pattern message")
    };
    assert!(matches!(
        p.pattern.parts(),
        [PatternPart::Text(Cow::Borrowed("Hello"))]
    ));
}

#[test]
fn relaxations_and_unknown_fields_are_accepted() {
    let text = r#"{
        "type": "message", "span": [0, 9],
        "pattern": [
            "a", "", "b",
            {"type": "expression", "function": {"type": "function", "name": "f"}},
            {"type": "markup", "kind": "open", "name": "x", "extra": 1}
        ]
    }"#;
    let m: Message<'_> = serde_json::from_str(text).expect("relaxed input accepted");
    let Message::Pattern(p) = m else {
        panic!("pattern message")
    };
    assert!(p.declarations.is_empty(), "{:?}", p.declarations);
    let parts = p.pattern.parts();
    assert_eq!(parts.len(), 3);
    // "a", "" and "b" normalize to one text part.
    assert_eq!(parts[0], PatternPart::Text("ab".into()));
    let PatternPart::Expression(e) = &parts[1] else {
        panic!("expression")
    };
    assert!(e.attributes().is_empty(), "{:?}", e.attributes());
    assert!(e.function().is_some_and(|f| f.options.is_empty()));
}

#[test]
fn malformed_input_is_rejected() {
    for bad in [
        r#"{"type": "nope", "pattern": []}"#,
        r#"{"type": "message"}"#,
        r#"{"type": "message", "pattern": [{"type": "expression"}]}"#,
        r#"{"type": "message", "pattern": [3]}"#,
        r#"{"type": "select", "declarations": [], "selectors": [{"type": "literal", "value": "x"}], "variants": []}"#,
        r#"{"type": "message", "declarations": [{"type": "input", "name": "a",
            "value": {"type": "expression", "arg": {"type": "variable", "name": "b"}}}], "pattern": []}"#,
        r#"{"type": "message", "declarations": [{"type": "input", "name": "a",
            "value": {"type": "expression", "arg": {"type": "literal", "value": "a"}}}], "pattern": []}"#,
        r#"{"type": "message", "pattern": [{"type": "expression", "arg": {"type": "literal", "value": "a"},
            "attributes": {"x": false}}]}"#,
        r#"{"type": "message", "pattern": [{"type": "markup", "kind": "empty", "name": "x"}]}"#,
    ] {
        assert!(serde_json::from_str::<Message<'_>>(bad).is_err(), "{bad}");
    }
}

#[test]
fn duplicate_option_names_cannot_be_serialized() {
    let mut options = Options::new();
    options.push("a".into(), OptionValue::Literal(lit("1")));
    options.push("a".into(), OptionValue::Literal(lit("2")));
    let f = FunctionRef {
        name: "f".into(),
        options,
    };
    assert!(serde_json::to_string(&f).is_err());

    // Distinct spellings are distinct JSON keys, even if NFC-equal.
    let mut options = Options::new();
    options.push("\u{e9}".into(), OptionValue::Literal(lit("1")));
    options.push("e\u{301}".into(), OptionValue::Literal(lit("2")));
    assert!(serde_json::to_string(&options).is_ok());
}

/// A repeated attribute name is valid syntax, and all but the last
/// occurrence are ignored (`syntax.md`, "Attributes"): JSON writes the last.
#[test]
fn a_repeated_attribute_name_serializes_its_last_occurrence() {
    let a = attrs(&[("x", None), ("y", Some("2")), ("x", Some("1"))]);
    assert_eq!(
        serde_json::to_value(&a).expect("serializable"),
        json!({"y": {"type": "literal", "value": "2"}, "x": {"type": "literal", "value": "1"}})
    );
    let a = attrs(&[("x", Some("1")), ("x", None)]);
    assert_eq!(
        serde_json::to_value(&a).expect("serializable"),
        json!({"x": true})
    );
}

#[test]
fn duplicate_json_members_are_kept_for_validation() {
    let text = r#"{"type": "function", "name": "f", "options": {
        "a": {"type": "literal", "value": "1"}, "a": {"type": "literal", "value": "2"}}}"#;
    let f: FunctionRef<'_> = serde_json::from_str(text).expect("deserializable");
    assert_eq!(f.options.len(), 2);
}
