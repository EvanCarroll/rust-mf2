//! The serializer (work order A5): canonical output, when a pattern must be
//! quoted, escaping, round trips, and the models MF2 syntax cannot represent.

use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionExpression, FunctionRef,
    InputDeclaration, Key, Literal, LiteralExpression, Message, Options, Pattern, PatternMessage,
    PatternPart, SelectMessage, VariableExpression, VariableRef, Variant,
};
use mf2_syntax::{Error, NameRole, parse_model, serialize};

fn round(src: &str) -> String {
    let m = parse_model(src).message.expect("parses");
    let out = serialize(&m).expect("serializes");
    assert_eq!(
        parse_model(&out).message.as_ref(),
        Some(&m),
        "{src:?} → {out:?} does not round-trip"
    );
    out
}

fn text_message(text: &str) -> Message<'_> {
    Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern: Pattern::from_text(text.into()),
    })
}

#[test]
fn canonical_forms() {
    let cases = [
        ("hello", "hello"),
        ("", ""),
        ("  spaced  ", "  spaced  "),
        (
            "{  $x  :number   minimumFractionDigits = 2  @a  }",
            "{$x :number minimumFractionDigits=2 @a}",
        ),
        ("{|plain|}", "{plain}"),
        ("{|two words|}", "{|two words|}"),
        ("{||}", "{||}"),
        ("{|a\\|b\\\\c|}", "{|a\\|b\\\\c|}"),
        ("{|\\{x\\}|}", "{|{x}|}"),
        ("a\\{b\\}c\\\\d\\|e", "a\\{b\\}c\\\\d|e"),
        ("{ #b a=1 @c }x{ /b }{#img/}", "{#b a=1 @c}x{/b}{#img /}"),
        ("{ :ns:fn o=$v }", "{:ns:fn o=$v}"),
        (
            ".input{$n :number}.local $m={$n}{{{$m}}}",
            ".input {$n :number}\n.local $m = {$n}\n{{{$m}}}",
        ),
        (
            ".input {$n :number} .match $n one {{One}} * {{Other {$n}}}",
            ".input {$n :number}\n.match $n\none {{One}}\n* {{Other {$n}}}",
        ),
        (
            ".local $x = {|a b| :string} .local $y = {1} .match $x $y |a b| 1 {{a}} * * {{b}}",
            ".local $x = {|a b| :string}\n.local $y = {1}\n.match $x $y\n|a b| 1 {{a}}\n* * {{b}}",
        ),
        ("  {{ padded }}  ", " padded "),
        ("{{}}", ""),
    ];
    for (src, want) in cases {
        assert_eq!(round(src), want, "{src:?}");
    }
}

#[test]
fn text_that_would_read_as_a_keyword_is_quoted() {
    assert_eq!(round("{{.input {$x}}}"), "{{.input {$x}}}");
    assert_eq!(round("{{ .local $x = {$y}}}"), "{{ .local $x = {$y}}}");
    assert_eq!(round("{{\u{200e}.x}}"), "{{\u{200e}.x}}");
    // A dot later in the text is fine.
    assert_eq!(round("a.b"), "a.b");
    assert_eq!(
        serialize(&text_message(".starts with a dot")).expect("serializes"),
        "{{.starts with a dot}}"
    );
}

#[test]
fn the_suite_forms_round_trip() {
    for src in [
        "\n hello\t",
        "  \u{61c} Hello world!",
        "{\u{1F954}}",
        "{\u{a1}\u{61d}\u{1681}\u{200b}\u{2010}\u{2030}\u{2060}\u{206a}\u{3001}\u{e000}\u{fdf0}}",
        ".local $\u{200e}foo\u{200f} = {3} {{{$\u{200e}foo\u{200f}}}}",
        "{#tag a:foo=|foo| b:bar=$bar}",
        "{42 @foo @bar=13}",
        "hello { world\t\n}",
        ".local $x = {1} {{ {\u{200e} $x \u{200f}} }}",
    ] {
        round(src);
    }
}

#[test]
fn a_catch_all_value_is_not_representable_and_is_dropped() {
    let m = Message::Select(SelectMessage {
        declarations: Vec::new(),
        selectors: vec![VariableRef { name: "x".into() }],
        variants: vec![Variant {
            keys: vec![Key::CatchAll(CatchAllKey {
                value: Some("other".into()),
            })],
            value: Pattern::from_text("o".into()),
        }],
    });
    assert_eq!(serialize(&m).expect("serializes"), ".match $x\n* {{o}}");
}

fn function_expr(name: &str) -> Expression<'_> {
    Expression::Function(FunctionExpression {
        function: FunctionRef {
            name: name.into(),
            options: Options::new(),
        },
        attributes: Attributes::new(),
    })
}

#[test]
fn unrepresentable_models_are_errors() {
    assert_eq!(serialize(&text_message("a\0b")), Err(Error::Nul));
    let literal_with_nul = Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern: vec![PatternPart::Expression(Expression::Literal(
            LiteralExpression {
                arg: Literal { value: "\0".into() },
                function: None,
                attributes: Attributes::new(),
            },
        ))]
        .into(),
    });
    assert_eq!(serialize(&literal_with_nul), Err(Error::Nul));
    for (name, role) in [
        ("1x", NameRole::Function),
        ("", NameRole::Function),
        ("a b", NameRole::Function),
        ("a:b:c", NameRole::Function),
    ] {
        let m = Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern: vec![PatternPart::Expression(function_expr(name))].into(),
        });
        assert_eq!(serialize(&m), Err(Error::InvalidName(role)), "{name:?}");
    }
    let bad_var = Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern: vec![PatternPart::Expression(Expression::Variable(
            VariableExpression {
                arg: VariableRef {
                    name: "ns:x".into(),
                },
                function: None,
                attributes: Attributes::new(),
            },
        ))]
        .into(),
    });
    assert_eq!(
        serialize(&bad_var),
        Err(Error::InvalidName(NameRole::Variable))
    );
    let mismatch = Message::Pattern(PatternMessage {
        declarations: vec![Declaration::Input(InputDeclaration {
            name: "a".into(),
            value: VariableExpression {
                arg: VariableRef { name: "b".into() },
                function: None,
                attributes: Attributes::new(),
            },
        })],
        pattern: Pattern::new(),
    });
    assert_eq!(serialize(&mismatch), Err(Error::InputNameMismatch));
    let select = |selectors: Vec<VariableRef<'static>>, variants: Vec<Variant<'static>>| {
        Message::Select(SelectMessage {
            declarations: Vec::new(),
            selectors,
            variants,
        })
    };
    let x = || VariableRef { name: "x".into() };
    let star = || Variant {
        keys: vec![Key::CatchAll(CatchAllKey::default())],
        value: Pattern::new(),
    };
    assert_eq!(
        serialize(&select(vec![], vec![star()])),
        Err(Error::NoSelectors)
    );
    assert_eq!(
        serialize(&select(vec![x()], vec![])),
        Err(Error::NoVariants)
    );
    assert_eq!(
        serialize(&select(
            vec![x()],
            vec![Variant {
                keys: vec![],
                value: Pattern::new()
            }]
        )),
        Err(Error::NoKeys)
    );
}
