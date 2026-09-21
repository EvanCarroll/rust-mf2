//! Writer → reader → decoder round trips (A4, A5): hand-written models of
//! every node kind, spellings that are not NFC, invalid models, every
//! parseable suite message, the whole reference workload as one catalog;
//! determinism; and what the writer refuses.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;

use mf2_catalog::writer::{Options, catalog, single};
use mf2_catalog::{Catalog, Dir, Manifest, MsgId, WriteError, decode, decode_report};
use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, InputDeclaration, Key, Literal,
    LiteralExpression, LocalDeclaration, Message, Pattern, PatternMessage, PatternPart,
    SelectMessage, VariableExpression, VariableRef, Variant,
};
use serde_json::json;

const ID: MsgId = MsgId::from_raw(0);

fn slots_of<'a>(m: &'a Message<'_>) -> Vec<Cow<'a, str>> {
    mf2_syntax::analyze(m)
        .externals
        .into_iter()
        .map(|n| n.nfc)
        .collect()
}

/// `single` → `Catalog::new` → `decode` must give `m` back, unstripped; and
/// the stripped catalog must decode to the same thing as the unstripped one
/// when the message has no COLD data.
fn round_trip(m: &Message<'_>) {
    let slots = slots_of(m);
    let slots: Vec<&str> = slots.iter().map(|s| &**s).collect();
    for options in [
        Options::new("en", Dir::Ltr),
        Options::new("ar", Dir::Rtl).stripped(),
    ] {
        let (bytes, manifest) = single(m, &slots, &options).unwrap();
        let again = single(m, &slots, &options).unwrap().0;
        assert_eq!(bytes, again, "not deterministic");
        let cat = Catalog::new(bytes, manifest.hash()).unwrap();
        assert!(Catalog::new(cat.as_bytes().to_vec(), manifest.hash() ^ 1).is_err());
        let d = decode_report(&cat, ID).unwrap();
        if options.strip_cold {
            if !d.cold_dropped {
                assert_eq!(&d.message, m, "stripped, no COLD data");
            }
        } else {
            assert!(!d.cold_dropped);
            assert_eq!(&d.message, m);
        }
    }
}

fn parse(src: &str) -> Message<'_> {
    let p = mf2_syntax::parse_model(src);
    p.message
        .unwrap_or_else(|| panic!("{src:?}: {:?}", p.diagnostics))
}

#[test]
fn every_node_kind() {
    let v = json!({
        "type": "select",
        "declarations": [
            {"type": "input", "name": "n", "value": {
                "type": "expression",
                "arg": {"type": "variable", "name": "n"},
                "function": {"type": "function", "name": "number",
                    "options": {"minimumFractionDigits": {"type": "literal", "value": "2"}}},
                "attributes": {}}},
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
    });
    let text = v.to_string();
    let m: Message<'_> = serde_json::from_str(&text).unwrap();
    round_trip(&m);
}

#[test]
fn sources_of_every_construct() {
    for src in [
        "",
        "Hello",
        "{{}}",
        "{{.dot}}",
        "Hi {$name}!",
        "{|lit|} {|a\\|b|} {1} {-1.5e3 :number}",
        "{:fn}{:ns:fn a=1 b=$x c=|q|}",
        "{$x :number minimumFractionDigits=2 u:dir=rtl u:id=x}",
        "{#b}bold{/b}{#img src=|a.png| alt=$alt /}",
        "{#a href=$url @can-copy @translate=no}x{/a @t}",
        "{$x @a @b=|1| @c=2}",
        ".local $x = {1} {{{$x}}}",
        ".local $x = {$y} .local $z = {$x :string} {{{$z}{$y}}}",
        ".input {$n :integer} .local $m = {$n} .match $n $m 1 * {{a}} * one {{b}} * * {{c}}",
        ".input {$x :string} .match $x |a b| {{A}} |\\|| {{B}} * {{*}}",
        ".match $x * {{no annotation}}",
        "{{  spaced  }}",
        "tab\tnewline\nemoji 🙂 rtl אב",
    ] {
        round_trip(&parse(src));
    }
}

#[test]
fn spellings_that_are_not_nfc() {
    // "é" composed (U+00E9) and decomposed (e + U+0301); Kelvin sign → K.
    for src in [
        "{$caf\u{e9}} {$cafe\u{301}}",
        "{$cafe\u{301}}",
        ".input {$cafe\u{301} :string} .match $caf\u{e9} caf\u{e9} {{1}} cafe\u{301} {{2}} * {{3}}",
        ".local $\u{212a} = {1} {{{$K} {$\u{212a}}}}",
        "{:ns:cafe\u{301}} {$x :number \u{212a}=1}",
        "{#cafe\u{301} \u{212a}=$cafe\u{301}}{/caf\u{e9}}",
    ] {
        round_trip(&parse(src));
    }
}

#[test]
fn invalid_models_round_trip() {
    // Data-model errors are representable; the format carries them.
    for src in [
        "{$x :number a=1 a=2}",
        ".local $x = {1} .local $x = {2} {{{$x}}}",
        ".local $x = {$x} {{{$x}}}",
        ".input {$x} .input {$x} {{}}",
        ".local $x = {1} .input {$x} {{{$x}}}",
        ".input {$x :string} .match $x 1 2 {{}} * {{}}",
        ".input {$x :string} .match $x * {{a}} * {{b}}",
        ".match $x 1 {{no fallback}}",
    ] {
        round_trip(&parse(src));
    }
    // Shapes MF2 syntax cannot write.
    let empty_select = Message::Select(SelectMessage {
        declarations: vec![],
        selectors: vec![],
        variants: vec![],
    });
    round_trip(&empty_select);
    let keyless = Message::Select(SelectMessage {
        declarations: vec![],
        selectors: vec![VariableRef { name: "x".into() }],
        variants: vec![Variant {
            keys: vec![],
            value: Pattern::from_text("t".into()),
        }],
    });
    round_trip(&keyless);
    let catch_all_values = Message::Select(SelectMessage {
        declarations: vec![],
        selectors: vec![VariableRef { name: "x".into() }],
        variants: vec![Variant {
            keys: vec![Key::CatchAll(CatchAllKey {
                value: Some("other".into()),
            })],
            value: Pattern::new(),
        }],
    });
    round_trip(&catch_all_values);
}

#[test]
fn suite_and_workload() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let suite: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("bench/corpora/suite.json")).unwrap(),
    )
    .unwrap();
    let mut n = 0;
    for t in &suite {
        let src = t["src"].as_str().unwrap();
        if let Some(m) = mf2_syntax::parse_model(src).message {
            round_trip(&m);
            n += 1;
        }
    }
    assert_eq!(n, 326);

    // The workload as one catalog with its manifest.
    let text = std::fs::read_to_string(root.join("bench/corpora/workload-1600.json")).unwrap();
    let corpus: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
    let models: Vec<Message<'_>> = corpus.values().map(|s| parse(s)).collect();
    let mut manifest = Manifest::default();
    let mut functions = std::collections::BTreeSet::new();
    for (id, m) in corpus.keys().zip(&models) {
        let a = mf2_syntax::analyze(m);
        manifest.ids.push(id.clone());
        manifest
            .slots
            .push(a.externals.iter().map(|x| x.nfc.to_string()).collect());
        manifest
            .markup
            .push(a.markup.iter().map(|x| x.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|x| x.nfc.to_string()));
    }
    manifest.functions = functions.into_iter().collect();
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
    let bytes = catalog(&manifest, &refs, &Options::new("en", Dir::Ltr)).unwrap();
    assert_eq!(
        bytes,
        catalog(&manifest, &refs, &Options::new("en", Dir::Ltr)).unwrap()
    );
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    for (i, (id, m)) in corpus.keys().zip(&models).enumerate() {
        let mid = MsgId::new(0, u32::try_from(i).unwrap()).unwrap();
        assert_eq!(cat.lookup(id), Some(mid), "{id}");
        assert_eq!(&decode(&cat, mid).unwrap(), m, "{id}");
    }
    assert_eq!(cat.lookup("zzz"), None);
    assert_eq!(cat.lookup(""), None);
}

#[test]
fn absent_fallback_and_chunks() {
    let a = parse("A {$x}");
    let b = parse("B");
    let manifest = Manifest {
        ids: vec!["a".into(), "b".into(), "c".into()],
        slots: vec![vec!["x".into()], vec![], vec![]],
        markup: vec![vec![], vec![], vec![]],
        functions: vec![],
    };
    let mut options = Options::new("es-MX", Dir::Ltr);
    options.chunk = 3;
    options.fallback = vec![(2, "en".into()), (0, "es".into())];
    let bytes = catalog(&manifest, &[Some(&a), None, Some(&b)], &options).unwrap();
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    assert_eq!(cat.chunk(), 3);
    let id = |i| MsgId::new(3, i).unwrap();
    assert_eq!(decode(&cat, id(0)).unwrap(), a);
    assert!(decode(&cat, id(1)).is_err());
    assert_eq!(decode(&cat, id(2)).unwrap(), b);
    assert!(decode(&cat, MsgId::new(0, 0).unwrap()).is_err());
    assert_eq!(cat.fallback_locale(id(0)), Some("es"));
    assert_eq!(cat.fallback_locale(id(1)), None);
    assert_eq!(cat.fallback_locale(id(2)), Some("en"));
    assert_eq!(cat.lookup("b"), Some(id(1)));
}

#[test]
fn many_ids_lookup() {
    let ids: Vec<String> = (0..1000)
        .map(|i| format!("group{}.item{:04}", i % 7, i))
        .collect();
    let mut ids = ids;
    ids.sort();
    let n = ids.len();
    let manifest = Manifest {
        ids: ids.clone(),
        slots: vec![vec![]; n],
        markup: vec![vec![]; n],
        functions: vec![],
    };
    let m = parse("x");
    let refs = vec![Some(&m); n];
    let bytes = catalog(&manifest, &refs, &Options::new("en", Dir::Ltr)).unwrap();
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(
            cat.lookup(id).map(MsgId::index),
            Some(u32::try_from(i).unwrap()),
            "{id}"
        );
        assert_eq!(cat.lookup(&format!("{id}x")), None);
        assert_eq!(cat.lookup(&id[..id.len() - 1]), None);
    }
    assert_eq!(cat.lookup("a"), None);
    assert_eq!(cat.lookup("zzzz"), None);
}

#[test]
fn what_the_writer_refuses() {
    let opts = Options::new("en", Dir::Ltr);
    let text = |t: &str| {
        Message::Pattern(PatternMessage {
            declarations: vec![],
            pattern: Pattern::from_text(t.to_owned().into()),
        })
    };
    assert!(matches!(
        single(&text("a\0b"), &[], &opts),
        Err(WriteError::Nul(_))
    ));
    assert!(matches!(
        single(&text("a"), &[], &Options::new("e\0n", Dir::Ltr)),
        Err(WriteError::Nul(_))
    ));
    assert!(matches!(
        single(&text("a"), &[], &Options::new("en", Dir::Auto)),
        Err(WriteError::Dir)
    ));
    // A NUL in COLD data is refused even when COLD is stripped.
    let mut attrs = Attributes::new();
    attrs.push("a".into(), Some(Literal { value: "\0".into() }));
    let with_attr = Message::Pattern(PatternMessage {
        declarations: vec![],
        pattern: Pattern::from(vec![PatternPart::Expression(Expression::Literal(
            LiteralExpression {
                arg: Literal { value: "x".into() },
                function: None,
                attributes: attrs,
            },
        ))]),
    });
    assert!(matches!(
        single(&with_attr, &[], &opts.clone().stripped()),
        Err(WriteError::Nul(_))
    ));
    // Unknown variable: a slot list that lacks it.
    assert!(matches!(
        single(&parse("{$x}"), &[], &opts),
        Err(WriteError::UnknownVariable { .. })
    ));
    // An .input whose name differs from its variable.
    let input = Message::Pattern(PatternMessage {
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
    assert!(matches!(
        single(&input, &["b"], &opts),
        Err(WriteError::InputName { .. })
    ));
    // A function outside the manifest's set.
    let m = parse("{$x :number}");
    let manifest = Manifest {
        ids: vec!["a".into()],
        slots: vec![vec!["x".into()]],
        markup: vec![vec![]],
        functions: vec!["integer".into()],
    };
    assert!(matches!(
        catalog(&manifest, &[Some(&m)], &opts),
        Err(WriteError::UnknownFunction { .. })
    ));
    // Manifest shape.
    let bad = Manifest {
        ids: vec!["b".into(), "a".into()],
        slots: vec![vec![], vec![]],
        markup: vec![vec![], vec![]],
        functions: vec![],
    };
    assert!(matches!(
        catalog(&bad, &[None, None], &opts),
        Err(WriteError::Manifest(_))
    ));
    // Locale entries and fallbacks.
    let mut o = opts.clone();
    o.locale_entries = vec![(1, vec![0x20])];
    assert!(matches!(
        single(&text("a"), &[], &o),
        Err(WriteError::LocaleEntry(1))
    ));
    let mut o = opts.clone();
    o.locale_entries = vec![(9, vec![]), (9, vec![1])];
    assert!(matches!(
        single(&text("a"), &[], &o),
        Err(WriteError::LocaleEntry(9))
    ));
    let mut o = opts.clone();
    o.fallback = vec![(0, "en".into()), (0, "fr".into())];
    assert!(matches!(
        single(&text("a"), &[], &o),
        Err(WriteError::Fallback(_))
    ));
    let mut o = opts;
    o.fallback = vec![(1, "en".into())];
    assert!(matches!(
        single(&text("a"), &[], &o),
        Err(WriteError::Fallback(_))
    ));
}

#[test]
fn local_shadowing_resolves_to_the_latest_binding() {
    // Two .locals of one name (a Duplicate Declaration), references between
    // and after them, and a declaration that refers to the external of its
    // own name.
    let m = Message::Pattern(PatternMessage {
        declarations: vec![
            Declaration::Local(LocalDeclaration {
                name: "x".into(),
                value: Expression::Variable(VariableExpression {
                    arg: VariableRef { name: "x".into() },
                    function: None,
                    attributes: Attributes::new(),
                }),
            }),
            Declaration::Local(LocalDeclaration {
                name: "x".into(),
                value: Expression::Variable(VariableExpression {
                    arg: VariableRef { name: "x".into() },
                    function: None,
                    attributes: Attributes::new(),
                }),
            }),
        ],
        pattern: Pattern::from(vec![PatternPart::Expression(Expression::Variable(
            VariableExpression {
                arg: VariableRef { name: "x".into() },
                function: None,
                attributes: Attributes::new(),
            },
        ))]),
    });
    round_trip(&m);
}

/// F8 across processes: the catalogs of every parseable suite message and
/// of the whole workload, written here and in a child process (this test
/// binary re-run), hash to the same digest.
#[test]
fn deterministic_across_processes() {
    const CHILD: &str = "MF2_CATALOG_DETERMINISM_CHILD";
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let digest = {
        // FNV-1a 64 over every catalog, each prefixed by its length.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |b: &[u8]| {
            for &x in (b.len() as u64).to_le_bytes().iter().chain(b) {
                h ^= u64::from(x);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        let suite: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join("bench/corpora/suite.json")).unwrap(),
        )
        .unwrap();
        for t in &suite {
            if let Some(m) = mf2_syntax::parse_model(t["src"].as_str().unwrap()).message {
                let slots = slots_of(&m);
                let slots: Vec<&str> = slots.iter().map(|s| &**s).collect();
                for o in [
                    Options::new("en", Dir::Ltr),
                    Options::new("en", Dir::Ltr).stripped(),
                ] {
                    feed(&single(&m, &slots, &o).unwrap().0);
                }
            }
        }
        let text = std::fs::read_to_string(root.join("bench/corpora/workload-1600.json")).unwrap();
        let corpus: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
        let models: Vec<Message<'_>> = corpus.values().map(|s| parse(s)).collect();
        let mut manifest = Manifest::default();
        let mut functions = std::collections::BTreeSet::new();
        for (id, m) in corpus.keys().zip(&models) {
            let a = mf2_syntax::analyze(m);
            manifest.ids.push(id.clone());
            manifest
                .slots
                .push(a.externals.iter().map(|x| x.nfc.to_string()).collect());
            manifest
                .markup
                .push(a.markup.iter().map(|x| x.nfc.to_string()).collect());
            functions.extend(a.functions.iter().map(|x| x.nfc.to_string()));
        }
        manifest.functions = functions.into_iter().collect();
        let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
        feed(&catalog(&manifest, &refs, &Options::new("en", Dir::Ltr)).unwrap());
        feed(&manifest.write());
        h
    };
    if std::env::var_os(CHILD).is_some() {
        println!("digest={digest:016x}");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "deterministic_across_processes",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let child = stdout
        .lines()
        .find_map(|l| {
            l.split_once("digest=")
                .map(|(_, d)| d.get(..16).unwrap_or(d))
        })
        .unwrap_or_else(|| panic!("child printed no digest: {stdout}"));
    assert_eq!(child, format!("{digest:016x}"));
}
