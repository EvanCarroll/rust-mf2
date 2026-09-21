//! The lossless CST: tokens tile the source, nodes nest, spans sit on char
//! boundaries — for valid and malformed input — and the diagnostic codes of
//! the main error situations are pinned (they are stable, see `code`).

use mf2_syntax::{Cst, Parser, SyntaxKind, code, parse_cst};

/// Valid and malformed messages covering every production and every recovery
/// path.
const INPUTS: &[&str] = &[
    "",
    "hello",
    "  \u{61c} Hello world!  ",
    "a\\{b\\}c\\\\d\\|e",
    "hello {$place-.} and {|lit\\|eral| :f k=v k2=|v 2| k3=$x @a @b=c @d=|e|}",
    "{#tag a:foo=|foo| b:bar=$bar}x{/tag @z}{#img /}{ #b/}",
    ".input {$n :number} .local $m = {$n :integer} .match $n $m one * {{a {$n}}} * * {{b}}",
    "\u{200e} .local $x = {1} {{ {$x}}} \u{2066}",
    ".local $\u{200e}foo\u{200f} = {5} {{{$foo}}}",
    "{:ns\u{200e}:\u{200e}fn}",
    "{1 :number minimumFractionDigits\u{200f}=\u{200e}1 }",
    "\u{3000}{{\u{3000}}}\u{3000}",
    // Malformed.
    "{",
    "}",
    "{}",
    "{{",
    "{{}",
    "{{}}}",
    "a\\",
    "a\\x",
    "a\0b",
    "{|unterminated",
    "{$}",
    "{::f}",
    "{:f opt}",
    "{:f opt=}",
    "{a @b=$c}",
    "{ @misplaced = attribute }",
    ".",
    ".foo {42} {{bar}}",
    ".local bar = {|foo|} {{_}}",
    ".local $x {1}",
    ".input {|lit|} {{}}",
    ".input {#m} {{}}",
    ".match {{foo}}",
    ".match $x* {{foo}}",
    ".match $x * {{foo}} extra",
    ".input {$x :x} .match $x * foo",
    "{{a}} tail",
    "{#a/ }",
    "{/a/}",
    "\u{0}\u{0}",
    "{\u{fdd0}}",
    "{:\u{10ffff}",
    "|||{{{}}}|||",
];

fn check_structure(cst: &Cst<'_>) {
    let src = cst.source();
    let view = cst.view();
    // Tokens tile the source, in order.
    let mut at = 0u32;
    for t in view.tokens() {
        let s = t.span();
        assert_eq!(
            s.start,
            at,
            "gap or overlap before {:?} in {src:?}",
            t.kind()
        );
        assert!(s.end > s.start, "empty token {:?} in {src:?}", t.kind());
        at = s.end;
    }
    assert_eq!(
        at as usize,
        src.len(),
        "tokens do not reach the end of {src:?}"
    );
    assert_eq!(cst.to_string(), src);
    // Nodes nest: every child lies inside its parent, children in order.
    let Some(root) = cst.root() else {
        panic!("no root for {src:?}")
    };
    assert_eq!(root.span().start, 0);
    assert_eq!(root.span().end as usize, src.len());
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let mut last = n.span().start;
        for c in n.children() {
            assert!(
                c.span().start >= last,
                "{:?} out of order in {src:?}",
                c.kind()
            );
            assert!(
                c.span().end <= n.span().end,
                "{:?} escapes its parent in {src:?}",
                c.kind()
            );
            last = c.span().end;
            stack.push(c);
        }
        let s = n.span();
        assert!(src.is_char_boundary(s.start as usize) && src.is_char_boundary(s.end as usize));
    }
    for d in cst.diagnostics() {
        let s = d.span.expect("syntax diagnostics have spans");
        assert!(s.start <= s.end && s.end as usize <= src.len());
        assert!(src.is_char_boundary(s.start as usize) && src.is_char_boundary(s.end as usize));
        assert!(
            code::describe(d.code).is_some(),
            "undocumented code {}",
            d.code
        );
    }
}

#[test]
fn every_input_gives_a_lossless_well_formed_tree() {
    for src in INPUTS {
        check_structure(&parse_cst(src));
    }
}

#[test]
fn a_reused_parser_gives_the_same_tree() {
    let mut parser = Parser::new();
    for src in INPUTS.iter().chain(INPUTS.iter().rev()) {
        let fresh = parse_cst(src);
        let reused = parser.parse_cst(src);
        assert_eq!(reused.nodes(), fresh.view().nodes(), "{src:?}");
        assert_eq!(reused.diagnostics(), fresh.diagnostics(), "{src:?}");
    }
}

fn codes(src: &str) -> Vec<u16> {
    parse_cst(src)
        .diagnostics()
        .iter()
        .map(|d| d.code)
        .collect()
}

#[test]
fn diagnostic_codes_are_pinned() {
    let cases: &[(&str, &[u16])] = &[
        ("hello", &[]),
        ("a\0b", &[code::NUL_CHARACTER]),
        ("a}b", &[code::UNESCAPED_CLOSE_BRACE]),
        ("a\\xb", &[code::INVALID_ESCAPE]),
        ("a\\", &[code::INVALID_ESCAPE]),
        ("{a", &[code::UNTERMINATED_PLACEHOLDER]),
        ("{}", &[code::EMPTY_PLACEHOLDER]),
        ("{a b}", &[code::UNEXPECTED_CHARACTER]),
        ("{42:func}", &[code::MISSING_WHITESPACE]),
        ("{:f@a}", &[code::MISSING_WHITESPACE]),
        ("{$1}", &[code::EXPECTED_NAME]),
        ("{:placeholder option}", &[code::EXPECTED_EQUALS]),
        ("{:placeholder option=}", &[code::EXPECTED_OPTION_VALUE]),
        ("{:f @a=$b}", &[code::EXPECTED_LITERAL]),
        (
            "{|abc",
            &[
                code::UNTERMINATED_QUOTED_LITERAL,
                code::UNTERMINATED_PLACEHOLDER,
            ],
        ),
        (".foo {{}}", &[code::UNKNOWN_KEYWORD]),
        (".local $x = |a| {{}}", &[code::EXPECTED_EXPRESSION]),
        (".input {42} {{}}", &[code::EXPECTED_VARIABLE_EXPRESSION]),
        (".local x = {1} {{}}", &[code::EXPECTED_VARIABLE]),
        (".local $x = {1}", &[code::MISSING_BODY]),
        ("{{a}} b", &[code::CONTENT_AFTER_BODY]),
        ("{{a", &[code::UNTERMINATED_QUOTED_PATTERN]),
        (".match * {{a}}", &[code::EXPECTED_SELECTOR]),
        (".input {$x :f} .match $x", &[code::EXPECTED_VARIANT]),
        (".input {$x :f} .match $x {{a}}", &[code::EXPECTED_KEY]),
        (
            ".input {$x :f} .match $x *",
            &[code::EXPECTED_QUOTED_PATTERN],
        ),
        (".local $x = {#m} {{}}", &[code::MARKUP_NOT_ALLOWED]),
        ("{@a}", &[code::EXPECTED_OPERAND]),
        // One diagnostic per error, not a cascade.
        (".local bar = {|foo|} {{_}}", &[code::EXPECTED_VARIABLE]),
        (
            ".local $foo\u{61c}bar = {2} {{ }}",
            &[code::EXPECTED_EQUALS],
        ),
        (
            "{:\u{10ffff}",
            &[code::EXPECTED_NAME, code::UNTERMINATED_PLACEHOLDER],
        ),
    ];
    for (src, want) in cases {
        assert_eq!(codes(src), *want, "{src:?}");
    }
}

#[test]
fn recovery_reports_errors_after_the_first() {
    // Two independent errors, both reported.
    assert_eq!(
        codes("{a b} and {$} and }"),
        [
            code::UNEXPECTED_CHARACTER,
            code::EXPECTED_NAME,
            code::UNESCAPED_CLOSE_BRACE
        ]
    );
    assert_eq!(
        codes(".foo {1} .local $x = {} {{a}b}}"),
        [
            code::UNKNOWN_KEYWORD,
            code::EMPTY_PLACEHOLDER,
            code::UNESCAPED_CLOSE_BRACE
        ]
    );
}

#[test]
fn navigation() {
    let cst = parse_cst("a {$x :f} b");
    let root = cst.root().expect("root");
    assert_eq!(root.kind(), SyntaxKind::SimpleMessage);
    let pattern = root.children().next().expect("pattern");
    assert_eq!(pattern.kind(), SyntaxKind::Pattern);
    let kinds: Vec<SyntaxKind> = pattern.children().map(|c| c.kind()).collect();
    assert_eq!(
        kinds,
        [SyntaxKind::Text, SyntaxKind::Expression, SyntaxKind::Text]
    );
    let expr = pattern.children().nth(1).expect("expression");
    assert_eq!(expr.text(), "{$x :f}");
    let names: Vec<&str> = expr
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::Name)
        .map(|n| n.text())
        .collect();
    assert_eq!(names, ["x", "f"]);
    assert!(expr.children().all(|c| c.span().start >= expr.span().start));
    assert!(SyntaxKind::Name.is_token() && SyntaxKind::Pattern.is_node());
    assert!(SyntaxKind::MarkupOpen.is_markup() && !SyntaxKind::Expression.is_markup());
}
