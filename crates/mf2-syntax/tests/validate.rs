//! Validation (work order A4): the six Data Model Errors, their codes and
//! spans from `parse_model`, NFC comparison of names and keys, and `validate`
//! on models built in code.

use mf2_model::{
    Attributes, CatchAllKey, Declaration, Diagnostic, ErrorKind, Expression, InputDeclaration, Key,
    Literal, LocalDeclaration, Message, Pattern, PatternMessage, SelectMessage, Span,
    VariableExpression, VariableRef, Variant,
};
use mf2_syntax::{code, parse_model, validate};

/// `(kind, code, spanned text)` of every data-model error of `src`.
fn errors(src: &str) -> Vec<(ErrorKind, u16, &str)> {
    let parsed = parse_model(src);
    assert!(parsed.message.is_some(), "{src:?} has a syntax error");
    parsed
        .diagnostics
        .iter()
        .map(|d| {
            let s = d.span.expect("parse_model gives spans");
            (d.kind, d.code, &src[s.start as usize..s.end as usize])
        })
        .collect()
}

#[test]
fn valid_messages_have_no_errors() {
    for src in [
        "hello {$x}",
        ".input {$n :number} .match $n one {{one}} * {{other}}",
        ".local $x = {1 :number} .match $x 1 {{one}} * {{other}}",
        ".input {$x :test:select} .local $y = {$x} .match $y 1 {{1}} * {{other}}",
        ".local $star = {star :string} .match $star |*| {{Literal star}} * {{The default}}",
        ".local $D\u{323}\u{307} = {foo} {{{$\u{1E0C}\u{307}}}}",
        ".input {$var :number maximumFractionDigits=0} .local $var2 = {$var :number maximumFractionDigits=2} .match $var2 0 {{a}} * {{b {$var :number maximumFractionDigits=3}}}",
        "{#b a=1 b=2}x{/b a=1}",
    ] {
        assert_eq!(errors(src), [], "{src:?}");
    }
}

#[test]
fn variant_key_mismatch_one_per_variant() {
    assert_eq!(
        errors(".input {$foo :x} .match $foo * * {{foo}}"),
        [(
            ErrorKind::VariantKeyMismatch,
            code::VARIANT_KEY_MISMATCH,
            "* *"
        )]
    );
    assert_eq!(
        errors(".input {$a :x} .input {$b :x} .match $a $b 1 {{x}} 2 {{y}} * * {{z}}"),
        [
            (
                ErrorKind::VariantKeyMismatch,
                code::VARIANT_KEY_MISMATCH,
                "1"
            ),
            (
                ErrorKind::VariantKeyMismatch,
                code::VARIANT_KEY_MISMATCH,
                "2"
            ),
        ]
    );
}

#[test]
fn missing_fallback_variant() {
    assert_eq!(
        errors(".input {$foo :x} .match $foo 1 {{_}}"),
        [(
            ErrorKind::MissingFallbackVariant,
            code::MISSING_FALLBACK_VARIANT,
            ".match"
        )]
    );
    // `|*|` is a literal, not the catch-all.
    assert_eq!(
        errors(".input {$foo :x} .match $foo |*| {{_}}"),
        [(
            ErrorKind::MissingFallbackVariant,
            code::MISSING_FALLBACK_VARIANT,
            ".match"
        )]
    );
}

#[test]
fn missing_selector_annotation_directly_or_through_locals() {
    for (src, selector) in [
        (".match $one 1 {{a}} * {{b}}", "$one"),
        (".input {$one} .match $one 1 {{a}} * {{b}}", "$one"),
        (
            ".local $one = {|The one|} .match $one 1 {{a}} * {{b}}",
            "$one",
        ),
        (
            ".input {$bar} .local $foo = {$bar} .match $foo one {{one}} * {{other}}",
            "$foo",
        ),
    ] {
        assert_eq!(
            errors(src),
            [(
                ErrorKind::MissingSelectorAnnotation,
                code::MISSING_SELECTOR_ANNOTATION,
                selector
            )],
            "{src:?}"
        );
    }
    // A chain that reaches a function is annotated.
    assert_eq!(
        errors(".input {$a :f} .local $b = {$a} .local $c = {$b} .match $c * {{x}}"),
        []
    );
}

#[test]
fn duplicate_declaration_both_rules() {
    let dup = (ErrorKind::DuplicateDeclaration, code::DUPLICATE_DECLARATION);
    let own = (
        ErrorKind::DuplicateDeclaration,
        code::SELF_REFERENCING_DECLARATION,
    );
    let cases: &[(&str, (ErrorKind, u16), &str)] = &[
        (".input {$foo} .input {$foo} {{_}}", dup, "$foo"),
        (".input {$foo} .local $foo = {42} {{_}}", dup, "$foo"),
        (".local $foo = {42} .input {$foo} {{_}}", dup, "$foo"),
        (".local $foo = {$bar} .local $bar = {42} {{_}}", dup, "$bar"),
        (
            ".local $foo = {42 :func opt=$bar} .local $bar = {42} {{_}}",
            dup,
            "$bar",
        ),
        (".local $foo = {$foo} {{_}}", own, "$foo"),
        (".local $foo = {42 :func opt=$foo} {{_}}", own, "$foo"),
        (".input {$foo :f opt=$foo} {{_}}", own, "$foo"),
    ];
    for (src, (kind, c), text) in cases {
        assert_eq!(errors(src), [(*kind, *c, *text)], "{src:?}");
    }
}

#[test]
fn duplicate_option_names_in_functions_and_markup() {
    let e = (ErrorKind::DuplicateOptionName, code::DUPLICATE_OPTION_NAME);
    assert_eq!(
        errors("bad {:placeholder option=x option=x}"),
        [(e.0, e.1, "option=x")]
    );
    assert_eq!(
        errors("bad {:placeholder ns:option=x ns:option=y}"),
        [(e.0, e.1, "ns:option=y")]
    );
    assert_eq!(
        errors(".local $foo = {horse :ns:func one=1 two=2 one=1} {{This is {$foo}}}"),
        [(e.0, e.1, "one=1")]
    );
    assert_eq!(errors("{#b k=1 k=2}"), [(e.0, e.1, "k=2")]);
    assert_eq!(
        errors(".input {$x :f} .match $x * {{{:g a=1 a=2}}}"),
        [(e.0, e.1, "a=2")]
    );
}

#[test]
fn duplicate_variant() {
    let e = (ErrorKind::DuplicateVariant, code::DUPLICATE_VARIANT);
    assert_eq!(
        errors(".input {$var :string} .match $var * {{a}} * {{b}}"),
        [(e.0, e.1, "*")]
    );
    assert_eq!(
        errors(
            ".input {$x :string} .input {$y :string} .match $x $y * foo {{a}} bar * {{b}} * |foo| {{c}} * * {{d}}"
        ),
        [(e.0, e.1, "* |foo|")]
    );
}

#[test]
fn names_and_keys_compare_under_nfc() {
    // functions/string.json #5–#7: NFC-equal keys are duplicates.
    for src in [
        ".local $x = {\u{1E0A}\u{323} :string} .match $x \u{1E0A}\u{323} {{a}} \u{1E0C}\u{307} {{b}} * {{c}}",
        ".local $x = {\u{1E0A}\u{323} :string} .match $x |\u{1E0A}\u{323}| {{a}} |\u{1E0C}\u{307}| {{b}} * {{c}}",
    ] {
        let e = errors(src);
        assert_eq!(e.len(), 1, "{src:?}");
        assert_eq!(e[0].0, ErrorKind::DuplicateVariant);
    }
    // NFC-equal declaration names are the same variable.
    assert_eq!(
        errors(".input {$\u{1E0C}\u{307}} .input {$D\u{323}\u{307}} {{_}}")[0].0,
        ErrorKind::DuplicateDeclaration
    );
    // NFC-equal option names are duplicates.
    assert_eq!(
        errors("{:f \u{e9}=1 e\u{301}=2}")[0].0,
        ErrorKind::DuplicateOptionName
    );
    // A selector reaches its declaration under NFC.
    assert_eq!(
        errors(".input {$\u{e9} :string} .match $e\u{301} a {{a}} * {{b}}"),
        []
    );
}

#[test]
fn several_errors_are_all_reported_in_source_order() {
    let e = errors(".input {$x} .input {$x} .match $x $y 1 {{a}} 1 {{b}}");
    let kinds: Vec<ErrorKind> = e.iter().map(|x| x.0).collect();
    assert_eq!(
        kinds,
        [
            ErrorKind::DuplicateDeclaration,
            ErrorKind::MissingFallbackVariant,
            ErrorKind::MissingSelectorAnnotation,
            ErrorKind::MissingSelectorAnnotation,
            ErrorKind::VariantKeyMismatch,
            ErrorKind::VariantKeyMismatch,
            ErrorKind::DuplicateVariant,
        ]
    );
    // Sorted by position.
    let parsed = parse_model(".input {$x} .input {$x} .match $x $y 1 {{a}} 1 {{b}}");
    let starts: Vec<u32> = parsed
        .diagnostics
        .iter()
        .map(|d| d.span.map_or(0, |s: Span| s.start))
        .collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "{starts:?}");
}

fn var_expr(name: &str) -> VariableExpression<'_> {
    VariableExpression {
        arg: VariableRef { name: name.into() },
        function: None,
        attributes: Attributes::new(),
    }
}

#[test]
fn validate_works_on_models_built_in_code() {
    let m = Message::Select(SelectMessage {
        declarations: vec![
            Declaration::Input(InputDeclaration {
                name: "x".into(),
                value: var_expr("x"),
            }),
            Declaration::Local(LocalDeclaration {
                name: "x".into(),
                value: Expression::Variable(var_expr("y")),
            }),
        ],
        selectors: vec![VariableRef { name: "x".into() }],
        variants: vec![
            Variant {
                keys: vec![Key::Literal(Literal { value: "a".into() })],
                value: Pattern::new(),
            },
            Variant {
                keys: vec![
                    Key::CatchAll(CatchAllKey::default()),
                    Key::CatchAll(CatchAllKey::default()),
                ],
                value: Pattern::new(),
            },
        ],
    });
    let d = validate(&m);
    let kinds: Vec<ErrorKind> = d.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [
            ErrorKind::DuplicateDeclaration,
            ErrorKind::MissingSelectorAnnotation,
            ErrorKind::VariantKeyMismatch,
        ]
    );
    assert!(d.iter().all(|x: &Diagnostic| x.span.is_none()));
    let ok = Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern: Pattern::from_text("fine".into()),
    });
    assert!(validate(&ok).is_empty());
}
