//! Lowering (work order A3): the data model a parse produces, which strings
//! stay borrowed from the source, bidi marks around names dropped, values
//! kept as written — and a model only when there is no syntax error.

use mf2_model::{
    Attributes, Cow, Declaration, ErrorKind, Expression, Frontend, FunctionRef, InputDeclaration,
    Key, Literal, LiteralExpression, LocalDeclaration, Markup, MarkupKind, Message, OptionValue,
    Options, Pattern, PatternMessage, PatternPart, VariableExpression, VariableRef,
};
use mf2_syntax::{Parser, parse_model};

fn model(src: &str) -> Message<'_> {
    let parsed = parse_model(src);
    assert!(
        !parsed.diagnostics.has(ErrorKind::Syntax),
        "{src:?}: {:?}",
        parsed.diagnostics
    );
    parsed.message.expect("a model")
}

fn pattern_of<'a>(m: &'a Message<'a>) -> &'a Pattern<'a> {
    match m {
        Message::Pattern(p) => &p.pattern,
        Message::Select(_) => panic!("pattern message expected"),
    }
}

// The point is to look at the `Cow` variant, so it takes the `Cow` itself.
#[allow(clippy::ptr_arg)]
fn is_borrowed(c: &Cow<'_, str>) -> bool {
    matches!(c, Cow::Borrowed(_))
}

fn var(name: &str) -> VariableRef<'_> {
    VariableRef { name: name.into() }
}

fn lit(value: &str) -> Literal<'_> {
    Literal {
        value: value.into(),
    }
}

#[test]
fn a_message_with_every_construct() {
    let src = ".input {$n :number minimumFractionDigits=2} \
               .local $m = {|a b| :string @t} \
               .match $n $m one * {{Hi {$n} {#b k=$m}x{/b}{:f}}} * * {{}}";
    let m = model(src);
    let Message::Select(s) = &m else {
        panic!("select message")
    };
    let mut options = Options::new();
    options.push(
        "minimumFractionDigits".into(),
        OptionValue::Literal(lit("2")),
    );
    assert_eq!(
        s.declarations[0],
        Declaration::Input(InputDeclaration {
            name: "n".into(),
            value: VariableExpression {
                arg: var("n"),
                function: Some(FunctionRef {
                    name: "number".into(),
                    options,
                }),
                attributes: Attributes::new(),
            },
        })
    );
    let mut attrs = Attributes::new();
    attrs.push("t".into(), None);
    assert_eq!(
        s.declarations[1],
        Declaration::Local(LocalDeclaration {
            name: "m".into(),
            value: Expression::Literal(LiteralExpression {
                arg: lit("a b"),
                function: Some(FunctionRef {
                    name: "string".into(),
                    options: Options::new(),
                }),
                attributes: attrs,
            }),
        })
    );
    assert_eq!(s.selectors, [var("n"), var("m")]);
    assert_eq!(s.variants.len(), 2);
    assert_eq!(s.variants[0].keys[0], Key::Literal(lit("one")));
    assert!(matches!(s.variants[0].keys[1], Key::CatchAll(ref c) if c.value.is_none()));
    let parts = s.variants[0].value.parts();
    assert_eq!(parts.len(), 7);
    assert_eq!(parts[0], PatternPart::Text("Hi ".into()));
    let mut markup_options = Options::new();
    markup_options.push("k".into(), OptionValue::Variable(var("m")));
    assert_eq!(
        parts[3],
        PatternPart::Markup(Markup {
            kind: MarkupKind::Open,
            name: "b".into(),
            options: markup_options,
            attributes: Attributes::new(),
        })
    );
    assert!(matches!(&parts[5], PatternPart::Markup(m) if m.kind == MarkupKind::Close));
    assert!(matches!(
        &parts[6],
        PatternPart::Expression(Expression::Function(_))
    ));
    assert!(s.variants[1].value.is_empty());
}

#[test]
fn strings_are_borrowed_unless_an_escape_changes_them() {
    let m = model("plain text");
    assert!(matches!(
        pattern_of(&m).parts(),
        [PatternPart::Text(Cow::Borrowed("plain text"))]
    ));
    // An escape next to other text: owned, cooked.
    let m = model("a\\{b");
    assert!(matches!(
        pattern_of(&m).parts(),
        [PatternPart::Text(Cow::Owned(t))] if t == "a{b"
    ));
    // A lone escape: the escaped character is in the source.
    let m = model("{{\\}}}");
    assert!(matches!(
        pattern_of(&m).parts(),
        [PatternPart::Text(Cow::Borrowed("}"))]
    ));
    let m = model("{|a b| :ns:fn opt=|x| @at=y} {$v}");
    let PatternPart::Expression(Expression::Literal(e)) = &pattern_of(&m).parts()[0] else {
        panic!("literal expression")
    };
    assert!(is_borrowed(&e.arg.value) && e.arg.value == "a b");
    let f = e.function.as_ref().expect("function");
    assert!(is_borrowed(&f.name) && f.name == "ns:fn");
    let (name, value) = f.options.iter().next().expect("option");
    assert_eq!(name, "opt");
    assert!(matches!(
        value,
        OptionValue::Literal(Literal {
            value: Cow::Borrowed("x")
        })
    ));
    assert!(matches!(
        e.attributes.get("at"),
        Some(Some(Literal {
            value: Cow::Borrowed("y")
        }))
    ));
    // A quoted literal with an escape is owned; an empty one is empty.
    let m = model("{|a\\|b|} {||}");
    let parts = pattern_of(&m).parts();
    let PatternPart::Expression(Expression::Literal(e)) = &parts[0] else {
        panic!("literal")
    };
    assert!(matches!(&e.arg.value, Cow::Owned(v) if v == "a|b"));
    let PatternPart::Expression(Expression::Literal(e)) = &parts[2] else {
        panic!("literal")
    };
    assert_eq!(e.arg.value, "");
}

#[test]
fn bidi_marks_around_names_are_dropped() {
    // bidi.json #23: `$‎foo‏` is `foo`.
    let m = model(".local $\u{200e}foo\u{200f} = {3} {{{$\u{200e}foo\u{200f}}}}");
    assert_eq!(m.declarations()[0].name(), "foo");
    let PatternPart::Expression(Expression::Variable(v)) = &pattern_of(&m).parts()[0] else {
        panic!("variable expression")
    };
    assert_eq!(v.arg.name, "foo");
    // Marks around a namespace's `:` make the identifier owned, still without
    // the marks.
    let m = model("{:ns\u{200e}:\u{200f}fn}");
    let PatternPart::Expression(e) = &pattern_of(&m).parts()[0] else {
        panic!("expression")
    };
    let name = &e.function().expect("function").name;
    assert!(matches!(name, Cow::Owned(n) if n == "ns:fn"));
    // bidi.json #19: marks around `=` are whitespace, not part of the value.
    let m = model("{1 :number minimumFractionDigits\u{200f}=\u{200e}1 }");
    let PatternPart::Expression(e) = &pattern_of(&m).parts()[0] else {
        panic!("expression")
    };
    let f = e.function().expect("function");
    assert_eq!(
        f.options.get("minimumFractionDigits"),
        Some(&OptionValue::Literal(lit("1")))
    );
}

#[test]
fn values_are_kept_as_written() {
    // syntax.json #108: no NFC in the model.
    let m = model(".local $D\u{323}\u{307} = {foo} {{{$\u{1E0C}\u{307}}}}");
    assert_eq!(m.declarations()[0].name(), "D\u{323}\u{307}");
    // Leading and trailing whitespace of a simple message is text.
    let m = model("\n hello\t");
    assert_eq!(pattern_of(&m).as_simple_text(), Some("\n hello\t"));
    let m = model("  \u{61c} Hello world!");
    assert_eq!(
        pattern_of(&m).as_simple_text(),
        Some("  \u{61c} Hello world!")
    );
    // ...but not of a complex message.
    let m = model("  {{ x }}  ");
    assert_eq!(pattern_of(&m).as_simple_text(), Some(" x "));
    // syntax.json #32: a quoted pattern may start with `.`.
    let m = model("{{.input {$x}}}");
    assert_eq!(pattern_of(&m).len(), 2);
}

#[test]
fn empty_and_whitespace_messages() {
    let m = model("");
    assert!(pattern_of(&m).is_empty());
    let m = model("{{}}");
    assert!(pattern_of(&m).is_empty());
    assert_eq!(
        model("   "),
        Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern: Pattern::from_text("   ".into()),
        })
    );
}

#[test]
fn no_model_with_a_syntax_error_and_every_syntax_error_reported() {
    let parsed = parse_model("{a b} {$}");
    assert!(parsed.message.is_none());
    assert_eq!(parsed.diagnostics.len(), 2);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|d| d.kind == ErrorKind::Syntax)
    );
    // Data-model errors keep the model.
    let parsed = parse_model(".input {$x} .input {$x} {{}}");
    assert!(parsed.message.is_some());
    assert!(parsed.diagnostics.has(ErrorKind::DuplicateDeclaration));
}

#[test]
fn a_reused_parser_agrees_with_fresh_parses() {
    let inputs = [
        "hello",
        "{$x :number}",
        ".input {$n :number} .match $n one {{one}} * {{other}}",
        "{",
        ".local $a = {1} .local $a = {2} {{}}",
        "",
        "  .x",
    ];
    let mut parser = Parser::new();
    for src in inputs.iter().chain(inputs.iter()) {
        assert_eq!(parser.parse(src), parse_model(src), "{src:?}");
        assert_eq!(parser.parse_model(src), parse_model(src), "{src:?}");
    }
}

#[test]
fn a_full_cst_lowers_to_the_same_model() {
    for src in [
        "a {$b :c d=e @f} g",
        ".local $x = {|y| :z} .match $x a {{b}} * {{c}}",
        ".input {$x} .input {$x} {{}}",
        "{",
    ] {
        assert_eq!(
            mf2_syntax::parse_cst(src).to_model(),
            parse_model(src),
            "{src:?}"
        );
    }
}
