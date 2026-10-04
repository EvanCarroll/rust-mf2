//! The reader on damaged input (A2): every `CatalogError`, truncation at
//! every byte, and every single-bit flip — each either rejected by
//! `Catalog::new` or loaded and then walked completely (views, strings,
//! names, functions, decoding) without a panic.

// Hand-edited headers of catalogs a few hundred bytes long: the casts cannot
// truncate, and the walk's helpers read best next to their one caller.
#![allow(clippy::cast_possible_truncation, clippy::items_after_statements)]

use mf2_catalog::writer::{Options, catalog};
use mf2_catalog::{
    Body, Catalog, CatalogError, DeclView, Dir, Entry, ExprView, KeyView, Manifest, MsgId, Operand,
    OptionsView, PartView, PatternView, decode_report, format::section,
};
use mf2_model::Message;

fn parse(src: &str) -> Message<'_> {
    mf2_syntax::parse_model(src).message.unwrap()
}

/// A catalog with every section and every construct: absent, simple,
/// pattern and select messages, attributes (COLD), fallbacks, plural
/// entries and an unknown LOCALE key, IDS.
fn rich() -> (Vec<u8>, u64) {
    let sources = [
        Some("Hello"),
        None,
        Some(".local $x = {$n :integer} {{{$x} {#b a=$n @t}x{/b} {|lit| :string} {:fn k=v}}}"),
        Some(".input {$n :integer} .match $n 1 {{one}} * {{{$n} other}}"),
        Some("{$caf\u{e9} @a=|1|} {$cafe\u{301}}"),
        Some(""),
    ];
    let models: Vec<Option<Message<'_>>> = sources.iter().map(|s| s.map(parse)).collect();
    let mut manifest = Manifest::default();
    let mut functions = std::collections::BTreeSet::new();
    for (i, m) in models.iter().enumerate() {
        manifest.ids.push(format!("m{i:02}"));
        let (slots, markup) = match m {
            Some(m) => {
                let a = mf2_syntax::analyze(m);
                functions.extend(a.functions.iter().map(|n| n.nfc.to_string()));
                (
                    a.externals.iter().map(|n| n.nfc.to_string()).collect(),
                    a.markup.iter().map(|n| n.nfc.to_string()).collect(),
                )
            }
            None => (vec![], vec![]),
        };
        manifest.slots.push(slots);
        manifest.markup.push(markup);
    }
    manifest.functions = functions.into_iter().collect();
    let mut options = Options::new("pl", Dir::Ltr);
    options.locale_entries = vec![
        (1, vec![0x21, 0x01, 0x05, 0x82, 0x01]),
        (2, vec![]),
        (77, vec![1, 2, 3]),
    ];
    options.fallback = vec![(0, "en".into()), (3, "de".into())];
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Option::as_ref).collect();
    let bytes = catalog(&manifest, &refs, &options).unwrap();
    (bytes, manifest.hash())
}

/// Touches everything a loaded catalog offers. Must never panic.
fn walk(cat: &Catalog) -> usize {
    let mut n = 0usize;
    let mut text = |r| {
        n += cat.text(r).map_or(0, str::len);
    };
    fn options(o: OptionsView<'_>, text: &mut impl FnMut(mf2_catalog::StrRef)) {
        for opt in o {
            let Ok((name, v)) = opt else { break };
            text(name);
            if let Operand::Literal(r) = v {
                text(r);
            }
        }
    }
    fn expr(e: ExprView<'_>, cat: &Catalog, text: &mut impl FnMut(mf2_catalog::StrRef)) {
        if let Some(Operand::Literal(r)) = e.operand() {
            text(r);
        }
        if let Some(f) = e.function() {
            let _ = cat.function(f.index());
            options(f.options(), text);
        }
    }
    fn pattern(p: PatternView<'_>, cat: &Catalog, text: &mut impl FnMut(mf2_catalog::StrRef)) {
        for part in p.parts() {
            match part {
                Ok(PartView::Text(r)) => text(r),
                Ok(PartView::Expression(e)) => expr(e, cat, text),
                Ok(PartView::Markup(m)) => {
                    text(m.name());
                    options(m.options(), text);
                }
                Err(_) => break,
            }
        }
    }
    let _ = (
        cat.locale(),
        cat.dir(),
        cat.cldr_version(),
        cat.sections().count(),
    );
    for k in 0..4 {
        let _ = cat.locale_entry(k);
    }
    for i in 0..cat.message_count().min(64) {
        let id = MsgId::new(cat.chunk(), i).unwrap();
        let _ = cat.fallback_locale(id);
        let names = cat.names(id);
        for s in 0..names.external_count().min(64) {
            if let Some(r) = names.external(s) {
                text(r);
            }
        }
        for s in 0..names.local_count().min(64) {
            if let Some(r) = names.local(s) {
                text(r);
            }
        }
        match cat.get(id) {
            Entry::Simple(r) => text(r),
            Entry::Pattern(v) | Entry::Select(v) => {
                let mut decls = v.declarations();
                for d in &mut decls {
                    match d {
                        Ok(DeclView::Input(e) | DeclView::Local { expr: e, .. }) => {
                            expr(e, cat, &mut text);
                        }
                        Err(_) => break,
                    }
                }
                match decls.body() {
                    Ok(Body::Pattern(p)) => pattern(p, cat, &mut text),
                    Ok(Body::Select(s)) => {
                        for _ in s.selectors() {}
                        for var in s.variants() {
                            let Ok(var) = var else { break };
                            for k in var.keys() {
                                if let Ok(KeyView::Literal(r)) = k {
                                    text(r);
                                }
                            }
                            pattern(var.pattern(), cat, &mut text);
                        }
                    }
                    Err(_) => {}
                }
            }
            Entry::Absent => {}
        }
        let _ = decode_report(cat, id);
    }
    for id in ["m00", "m03", "m05", "zz", ""] {
        let _ = cat.lookup(id);
    }
    let map = cat.nfc_map();
    n += map.code_points() + map.len_bytes();
    for ch in ['a', '\u{e9}', '\u{301}', '\u{ac01}', '\u{10FFFF}'] {
        if let Some(d) = map.decomposition(ch) {
            n += d.count();
        }
    }
    n
}

#[test]
fn the_rich_catalog_loads_and_walks() {
    let (bytes, hash) = rich();
    let cat = Catalog::new(bytes, hash).unwrap();
    assert!(walk(&cat) > 0);
    for i in 0..6 {
        let id = MsgId::new(0, i).unwrap();
        assert_eq!(cat.lookup(&format!("m{i:02}")), Some(id));
        let d = decode_report(&cat, id);
        assert_eq!(d.is_ok(), i != 1, "message {i}");
    }
    assert_eq!(cat.fallback_locale(MsgId::new(0, 3).unwrap()), Some("de"));
    assert_eq!(cat.locale_entry(77), Some(&[1u8, 2, 3][..]));
    assert_eq!(cat.locale_entry(2), Some(&[][..]));
}

/// The NFC section (`plan/01` §4.3): written when a code point outside the
/// catalog's own keys and names can reach one, read back as that map, and
/// left out entirely when nothing can.
#[test]
fn the_canonical_equivalence_map_round_trips() {
    let (bytes, hash) = rich();
    let cat = Catalog::new(bytes, hash).unwrap();
    // `rich` has the argument name `café`, so U+00E9 is reachable.
    let map = cat.nfc_map();
    assert!(!map.is_empty() && map.len_bytes() > 0);
    let decomp: Vec<char> = map.decomposition('\u{e9}').unwrap().collect();
    assert_eq!(decomp, ['e', '\u{301}']);
    assert!(map.decomposition('\u{df}').is_none(), "ß reaches nothing");
    assert!(
        cat.sections()
            .any(|(kind, _, len)| kind == section::NFC && len as usize == map.len_bytes())
    );

    // An ASCII-only catalog gets no section at all.
    let message = parse(".match $n 1 {{one}} * {{other}}");
    let manifest = Manifest {
        ids: vec!["a".into()],
        slots: vec![vec!["n".into()]],
        markup: vec![vec![]],
        functions: vec![],
    };
    let options = Options::new("en", Dir::Ltr);
    let bytes = catalog(&manifest, &[Some(&message)], &options).unwrap();
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    assert!(!cat.sections().any(|(kind, _, _)| kind == section::NFC));
    assert!(cat.nfc_map().is_empty());
}

#[test]
fn every_truncation_is_rejected() {
    let (bytes, hash) = rich();
    for len in 0..bytes.len() {
        assert!(
            Catalog::new(bytes[..len].to_vec(), hash).is_err(),
            "prefix of {len} bytes loaded"
        );
    }
}

#[test]
fn every_bit_flip_is_rejected_or_walks() {
    let (bytes, hash) = rich();
    let (mut loaded, mut rejected) = (0, 0);
    for at in 0..bytes.len() {
        for bit in 0..8 {
            let mut b = bytes.clone();
            b[at] ^= 1 << bit;
            // The stored hash, so that flips past the skew check are tested.
            let h = u64::from_le_bytes(b[8..16].try_into().unwrap());
            match Catalog::new(b, h) {
                Ok(cat) => {
                    walk(&cat);
                    loaded += 1;
                }
                Err(_) => rejected += 1,
            }
        }
    }
    assert!(
        loaded > 0 && rejected > 0,
        "{loaded} loaded, {rejected} rejected"
    );
    let _ = hash;
}

/// The section table of `b`: (index in the table, kind, offset, length).
fn table(b: &[u8]) -> Vec<(usize, u16, usize, usize)> {
    let n = u16::from_le_bytes([b[30], b[31]]) as usize;
    (0..n)
        .map(|i| {
            let at = 32 + 10 * i;
            (
                i,
                u16::from_le_bytes([b[at], b[at + 1]]),
                u32::from_le_bytes(b[at + 2..at + 6].try_into().unwrap()) as usize,
                u32::from_le_bytes(b[at + 6..at + 10].try_into().unwrap()) as usize,
            )
        })
        .collect()
}

fn find(b: &[u8], kind: u16) -> (usize, usize, usize) {
    let (i, _, off, len) = table(b).into_iter().find(|e| e.1 == kind).unwrap();
    (i, off, len)
}

fn set_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn expect(b: Vec<u8>, hash: u64, e: CatalogError) {
    assert_eq!(Catalog::new(b, hash).map(|_| ()), Err(e));
}

#[test]
fn every_catalog_error() {
    let (good, hash) = rich();
    let edit = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = good.clone();
        f(&mut b);
        b
    };
    expect(edit(&|b| b[3] = b'X'), hash, CatalogError::Magic);
    expect(b"MF2".to_vec(), hash, CatalogError::Magic);
    expect(edit(&|b| b[5] = 3), hash, CatalogError::Version);
    // Version 1 wrote no canonical-equivalence map, so 3.0 refuses it.
    expect(edit(&|b| b[5] = 1), hash, CatalogError::Version);
    // A minor bump is fine.
    assert!(Catalog::new(edit(&|b| b[4] = 7), hash).is_ok());
    expect(good[..5].to_vec(), hash, CatalogError::Truncated);
    expect(good[..31].to_vec(), hash, CatalogError::Truncated);
    expect(good[..40].to_vec(), hash, CatalogError::Truncated);
    expect(good.clone(), hash ^ 1, CatalogError::ManifestMismatch);
    expect(edit(&|b| b[29] = 2), hash, CatalogError::Header);
    expect(
        edit(&|b| set_u32(b, 16, (1 << 24) + 1)),
        hash,
        CatalogError::Header,
    );
    expect(
        edit(&|b| set_u32(b, 20, 1 << 20)),
        hash,
        CatalogError::Header,
    );

    // Section table.
    let (mi, moff, _) = find(&good, section::MESSAGES);
    expect(
        edit(&|b| set_u32(b, 32 + 10 * mi + 2, moff as u32 - 1)),
        hash,
        CatalogError::SectionTable,
    );
    let (si, _, slen) = find(&good, section::STRINGS);
    expect(
        edit(&|b| set_u32(b, 32 + 10 * si + 6, slen as u32 + 1)),
        hash,
        CatalogError::SectionTable,
    );
    expect(
        edit(&|b| {
            b.push(0);
        }),
        hash,
        CatalogError::SectionTable,
    );
    let (ni, _, _) = find(&good, section::NAMES);
    let (fi, _, _) = find(&good, section::FUNCS);
    // NAMES relabelled FUNCS: a known kind twice.
    expect(
        edit(&|b| b[32 + 10 * ni..32 + 10 * ni + 2].copy_from_slice(&section::FUNCS.to_le_bytes())),
        hash,
        CatalogError::SectionTable,
    );
    // STRINGS relabelled unknown and FUNCS relabelled STRINGS: STRINGS not last.
    expect(
        edit(&|b| {
            b[32 + 10 * si..32 + 10 * si + 2].copy_from_slice(&99u16.to_le_bytes());
            b[32 + 10 * fi..32 + 10 * fi + 2].copy_from_slice(&section::STRINGS.to_le_bytes());
        }),
        hash,
        CatalogError::SectionTable,
    );
    for kind in [
        section::INDEX,
        section::MESSAGES,
        section::NAMES,
        section::LOCALE,
        section::FUNCS,
        section::STRINGS,
    ] {
        let (i, _, _) = find(&good, kind);
        expect(
            edit(&|b| b[32 + 10 * i..32 + 10 * i + 2].copy_from_slice(&99u16.to_le_bytes())),
            hash,
            CatalogError::MissingSection,
        );
    }
    // Optional sections may go.
    for kind in [section::COLD, section::FALLBACK, section::IDS] {
        let (i, _, _) = find(&good, kind);
        let b = edit(&|b| b[32 + 10 * i..32 + 10 * i + 2].copy_from_slice(&99u16.to_le_bytes()));
        let cat = Catalog::new(b, hash).unwrap();
        walk(&cat);
    }

    // STRINGS must end with NUL.
    expect(
        edit(&|b| {
            let last = b.len() - 1;
            b[last] = b'x';
        }),
        hash,
        CatalogError::Strings,
    );

    // INDEX: a pattern entry past MESSAGES; entries out of order.
    let (_, ioff, ilen) = find(&good, section::INDEX);
    let n = ilen / 4;
    expect(edit(&|b| b[ioff + 3 * n] = 0x7f), hash, CatalogError::Index);
    expect(
        edit(&|b| {
            // Message 3 (select) to point where message 2 (pattern) does.
            for p in 0..3 {
                b[ioff + p * n + 3] = b[ioff + p * n + 2];
            }
        }),
        hash,
        CatalogError::Index,
    );

    // NAMES: a string reference past the pool.
    let (_, noff, _) = find(&good, section::NAMES);
    expect(
        edit(&|b| set_u32(b, noff + 2, 1 << 20)),
        hash,
        CatalogError::Names,
    );
    // FUNCS: a string reference past the pool.
    let (_, foff, _) = find(&good, section::FUNCS);
    expect(
        edit(&|b| set_u32(b, foff, 1 << 20)),
        hash,
        CatalogError::Funcs,
    );
    // FALLBACK: a locale index out of range.
    let (_, fboff, fblen) = find(&good, section::FALLBACK);
    expect(
        edit(&|b| b[fboff + fblen - 1] = 9),
        hash,
        CatalogError::Fallback,
    );
    // LOCALE: a malformed plural entry (rule header with zero groups).
    let (_, loff, _) = find(&good, section::LOCALE);
    expect(edit(&|b| b[loff + 3] = 0x20), hash, CatalogError::Locale);
    // IDS: a wrong restart offset.
    let (_, idoff, _) = find(&good, section::IDS);
    expect(edit(&|b| b[idoff] = 1), hash, CatalogError::Ids);
    // NFC: a count the section's length cannot hold.
    let (_, noff, _) = find(&good, section::NFC);
    expect(edit(&|b| b[noff] = 0xFF), hash, CatalogError::Nfc);
}

#[test]
fn unknown_sections_are_skipped() {
    // Append an unknown section before STRINGS by rebuilding the table.
    let (good, hash) = rich();
    let t = table(&good);
    let n = t.len();
    let (_, _, soff, slen) = t[n - 1];
    let mut b = Vec::new();
    b.extend_from_slice(&good[..30]);
    b.extend_from_slice(&((n + 1) as u16).to_le_bytes());
    let extra = [0xAAu8; 7];
    let shift = 10; // one more table entry
    for (_, kind, off, len) in &t[..n - 1] {
        b.extend_from_slice(&kind.to_le_bytes());
        b.extend_from_slice(&((off + shift) as u32).to_le_bytes());
        b.extend_from_slice(&(*len as u32).to_le_bytes());
    }
    b.extend_from_slice(&42u16.to_le_bytes());
    b.extend_from_slice(&((soff + shift) as u32).to_le_bytes());
    b.extend_from_slice(&(extra.len() as u32).to_le_bytes());
    b.extend_from_slice(&section::STRINGS.to_le_bytes());
    b.extend_from_slice(&((soff + shift + extra.len()) as u32).to_le_bytes());
    b.extend_from_slice(&(slen as u32).to_le_bytes());
    b.extend_from_slice(&good[32 + 10 * n..soff]);
    b.extend_from_slice(&extra);
    b.extend_from_slice(&good[soff..]);
    let cat = Catalog::new(b, hash).unwrap();
    assert_eq!(cat.sections().count(), n + 1);
    let original = Catalog::new(good, hash).unwrap();
    for i in [0, 2, 3, 4, 5] {
        let id = MsgId::new(0, i).unwrap();
        assert_eq!(
            decode_report(&cat, id).unwrap(),
            decode_report(&original, id).unwrap()
        );
    }
}

/// Whether a message calls a date function is read from its tags and FUNCS,
/// without formatting it (`plan/08` §4.3): a call in a placeholder, in a
/// declaration, in one variant, or of a function the application marked as
/// a date function counts; a bare placeholder does not.
#[test]
fn a_date_message_is_read_from_its_tags() {
    let is_date = |name: &str| matches!(name, "datetime" | "date" | "time" | "app:when");
    let check = |src: &str| -> (bool, bool) {
        let model = parse(src);
        let a = mf2_syntax::analyze(&model);
        let slots: Vec<&str> = a.externals.iter().map(|n| &*n.nfc).collect();
        let (bytes, manifest) =
            mf2_catalog::writer::single(&model, &slots, &Options::new("en", Dir::Ltr)).unwrap();
        let cat = Catalog::new(bytes, manifest.hash()).unwrap();
        let id = MsgId::new(0, 0).unwrap();
        (cat.names_function(is_date), cat.calls(id, is_date))
    };
    // A call in a placeholder.
    assert_eq!(check("Due {$d :datetime}"), (true, true));
    // A call in a declaration only: the placeholder itself is bare.
    assert_eq!(check(".input {$d :date} {{Due {$d}}}"), (true, true));
    assert_eq!(check(".local $t = {$d :time} {{At {$t}}}"), (true, true));
    // A call in one variant of a select.
    assert_eq!(
        check(".input {$n :integer} .match $n 1 {{one}} * {{{$d :date}}}"),
        (true, true)
    );
    // A function the application registered as a date function.
    assert_eq!(check("Due {$d :app:when}"), (true, true));
    // None: a number, a string, markup, a bare placeholder, plain text.
    assert_eq!(
        check("{$n :integer} {$s :string} {#b}x{/b} {$d}"),
        (false, false)
    );
    assert_eq!(check("Hello"), (false, false));

    // Per message, in a catalog of several.
    let (bytes, hash) = rich();
    let cat = Catalog::new(bytes, hash).unwrap();
    assert!(!cat.names_function(is_date));
    let integer = |name: &str| name == "integer";
    assert!(cat.names_function(integer));
    let calls: Vec<bool> = (0..6)
        .map(|i| cat.calls(MsgId::new(0, i).unwrap(), integer))
        .collect();
    assert_eq!(calls, [false, false, true, true, false, false]);
}
