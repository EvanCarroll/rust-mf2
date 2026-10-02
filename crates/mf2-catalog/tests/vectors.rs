//! The hand-worked vectors of `plans/02-catalog-format.md` §2.10: the writer
//! produces exactly these bytes, and the reader and decoder read these
//! values back.

// `m`, `p`, `e`, `f`, `d`: the message and its parts, as the format names them.
#![allow(clippy::many_single_char_names)]

use mf2_catalog::writer::{Options, single};
use mf2_catalog::{
    Body, Catalog, CldrVersion, DeclView, Dir, Entry, KeyView, MsgId, Operand, PartView, VarRef,
    decode, decode_report,
};
use mf2_model::Message;

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn model(src: &str) -> Message<'_> {
    let parsed = mf2_syntax::parse_model(src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.message.unwrap()
}

fn compile(m: &Message<'_>, options: &Options) -> (Vec<u8>, u64) {
    let a = mf2_syntax::analyze(m);
    let slots: Vec<&str> = a.externals.iter().map(|n| &*n.nfc).collect();
    let (bytes, manifest) = single(m, &slots, options).unwrap();
    (bytes, manifest.hash())
}

const V1: &str = "
    4D 46 32 42  00 02  03 00  EC 7E DC A7 AE 6C 0D D8
    01 00 00 00  00 00 00 00  00 00 00 00  00 00  06 00
    01 00 5C 00 00 00 04 00 00 00
    02 00 60 00 00 00 00 00 00 00
    04 00 60 00 00 00 00 00 00 00
    06 00 60 00 00 00 01 00 00 00
    07 00 61 00 00 00 00 00 00 00
    0F 00 61 00 00 00 09 00 00 00
    03 00 00 00
    00
    65 6E 00 48 65 6C 6C 6F 00";

const V2: &str = "
    4D 46 32 42  00 02  00 00  28 06 D0 17 9E ED 37 AC
    01 00 00 00  00 00 00 00  00 00 00 00  00 00  08 00
    01 00 70 00 00 00 04 00 00 00
    02 00 74 00 00 00 0C 00 00 00
    03 00 80 00 00 00 06 00 00 00
    04 00 86 00 00 00 06 00 00 00
    06 00 8C 00 00 00 01 00 00 00
    07 00 8D 00 00 00 04 00 00 00
    08 00 91 00 00 00 06 00 00 00
    0F 00 97 00 00 00 17 00 00 00
    00 00 00 40
    01 01 00 03 00 13 31 00 00 00 00 11
    01 00 01 01 0F 00
    01 00 0A 00 00 00
    00
    03 00 00 00
    00 00 00 00 00 00
    65 6E 00 73 74 72 69 6E 67 00 75 73 65 72 00 78 00 21 00 48 69 20 00";

const V3: &str = "
    4D 46 32 42  00 02  03 00  E2 3D 9F F4 10 B9 4F 20
    01 00 00 00  02 00 00 00  01 02 30 00  00 00  06 00
    01 00 5C 00 00 00 04 00 00 00
    02 00 60 00 00 00 17 00 00 00
    04 00 77 00 00 00 06 00 00 00
    06 00 7D 00 00 00 08 00 00 00
    07 00 85 00 00 00 04 00 00 00
    0F 00 89 00 00 00 1A 00 00 00
    00 00 00 80
    01 02 31 00 00 00 01 00 02 01 01 03 01 00 16 01 00 05 02 11 00 00 0F
    01 00 0D 00 00 00
    01 01 05 21 01 05 82 01
    05 00 00 00
    31 00 65 6E 00 69 6E 74 65 67 65 72 00 6E 00 20 69 74 65 6D 73 00 6F 6E 65 00";

const ID: MsgId = MsgId::from_raw(0);

#[test]
fn v1_simple() {
    let m = model("Hello");
    let (bytes, hash) = compile(&m, &Options::new("en", Dir::Ltr).stripped());
    assert_eq!(hash, 0xd80d_6cae_a7dc_7eec);
    assert_eq!(bytes, hex(V1));
    let cat = Catalog::new(bytes, hash).unwrap();
    assert_eq!(cat.format_version(), 0x0200);
    assert_eq!(cat.locale(), "en");
    assert_eq!(cat.dir(), Dir::Ltr);
    assert_eq!(cat.message_count(), 1);
    assert!(cat.cold_stripped() && cat.ids_stripped());
    assert_eq!(cat.lookup(""), None);
    let Entry::Simple(r) = cat.get(ID) else {
        panic!("not simple")
    };
    assert_eq!(cat.text(r), Some("Hello"));
    assert_eq!(decode(&cat, ID).unwrap(), m);
}

#[test]
fn v2_pattern() {
    let m = model("Hi {$user :string @x}!");
    let (bytes, hash) = compile(&m, &Options::new("en", Dir::Ltr));
    assert_eq!(hash, 0xac37_ed9e_17d0_0628);
    assert_eq!(bytes, hex(V2));
    let cat = Catalog::new(bytes, hash).unwrap();
    assert!(!cat.cold_stripped() && !cat.ids_stripped());
    assert_eq!(cat.lookup(""), Some(ID));
    assert_eq!(cat.lookup("x"), None);
    assert_eq!(cat.function(0), Some("string"));
    let names = cat.names(ID);
    assert_eq!(names.external_count(), 1);
    assert_eq!(cat.text(names.external(0).unwrap()), Some("user"));
    let Entry::Pattern(view) = cat.get(ID) else {
        panic!("not a pattern")
    };
    assert_eq!(view.declarations().len(), 0);
    let Body::Pattern(p) = view.body().unwrap() else {
        panic!("not a pattern body")
    };
    let parts: Vec<_> = p.parts().map(Result::unwrap).collect();
    assert_eq!(parts.len(), 3);
    let PartView::Expression(e) = parts[1] else {
        panic!("not an expression")
    };
    assert_eq!(e.operand(), Some(Operand::Variable(VarRef::External(0))));
    let f = e.function().unwrap();
    assert_eq!(cat.function(f.index()), Some("string"));
    assert!(f.options().is_empty());
    // Unstripped: the attribute comes back.
    let d = decode_report(&cat, ID).unwrap();
    assert!(!d.cold_dropped);
    assert_eq!(d.message, m);
}

#[test]
fn v2_stripped_drops_the_attribute_and_says_so() {
    let m = model("Hi {$user :string @x}!");
    let (bytes, hash) = compile(&m, &Options::new("en", Dir::Ltr).stripped());
    let cat = Catalog::new(bytes, hash).unwrap();
    let d = decode_report(&cat, ID).unwrap();
    assert!(d.cold_dropped);
    assert_eq!(d.message, model("Hi {$user :string}!"));
}

#[test]
fn v3_select() {
    let m = model(".input {$n :integer} .match $n 1 {{one}} * {{{$n} items}}");
    let mut options = Options::new("en", Dir::Ltr).stripped();
    options.cldr_version = Some(CldrVersion {
        major: 48,
        minor: 2,
        patch: 1,
    });
    options.locale_entries = vec![(1, vec![0x21, 0x01, 0x05, 0x82, 0x01])];
    let (bytes, hash) = compile(&m, &options);
    assert_eq!(hash, 0x204f_b910_f49f_3de2);
    assert_eq!(bytes, hex(V3));
    let cat = Catalog::new(bytes, hash).unwrap();
    assert_eq!(
        cat.cldr_version().map(CldrVersion::to_u32),
        Some(0x0030_0201)
    );
    assert_eq!(
        cat.locale_entry(1),
        Some(&[0x21, 0x01, 0x05, 0x82, 0x01][..])
    );
    assert_eq!(cat.locale_entry(2), None);
    assert_eq!(cat.function(0), Some("integer"));
    let Entry::Select(view) = cat.get(ID) else {
        panic!("not a select")
    };
    let decls: Vec<_> = view.declarations().map(Result::unwrap).collect();
    assert_eq!(decls.len(), 1);
    let DeclView::Input(e) = decls[0] else {
        panic!("not .input")
    };
    assert_eq!(e.operand(), Some(Operand::Variable(VarRef::External(0))));
    let Body::Select(s) = view.body().unwrap() else {
        panic!("not a select body")
    };
    let sel: Vec<_> = s.selectors().map(Result::unwrap).collect();
    assert_eq!(sel, [VarRef::External(0)]);
    let variants: Vec<_> = s.variants().map(Result::unwrap).collect();
    assert_eq!(variants.len(), 2);
    let k0: Vec<_> = variants[0].keys().map(Result::unwrap).collect();
    let KeyView::Literal(r) = k0[0] else {
        panic!("not a literal key")
    };
    assert_eq!(cat.text(r), Some("1"));
    let k1: Vec<_> = variants[1].keys().map(Result::unwrap).collect();
    assert_eq!(k1, [KeyView::CatchAll]);
    assert_eq!(variants[1].pattern().len(), 2);
    assert_eq!(decode(&cat, ID).unwrap(), m);
}
