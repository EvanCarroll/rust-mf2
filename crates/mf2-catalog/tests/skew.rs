//! Skew and flags (A10): F6 — a catalog compiled against another manifest
//! is rejected; what the manifest hash covers and what it does not; F7 —
//! fallback locales round-trip; F9 — an unknown major version is rejected;
//! stripping keeps the formatting-relevant model.

use std::collections::BTreeMap;
use std::path::Path;

use mf2_catalog::writer::{Options, catalog};
use mf2_catalog::{Catalog, CatalogError, Dir, Manifest, MsgId, decode, decode_report};
use mf2_model::Message;

fn parse(src: &str) -> Message<'_> {
    mf2_syntax::parse_model(src).message.unwrap()
}

/// The manifest of `sources` (ids sorted), as the build derives it.
fn manifest_of(ids: &[&str], models: &[Message<'_>]) -> Manifest {
    let mut m = Manifest::default();
    let mut functions = std::collections::BTreeSet::new();
    for (id, model) in ids.iter().zip(models) {
        let a = mf2_syntax::analyze(model);
        m.ids.push((*id).to_owned());
        m.slots
            .push(a.externals.iter().map(|n| n.nfc.to_string()).collect());
        m.markup
            .push(a.markup.iter().map(|n| n.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|n| n.nfc.to_string()));
    }
    m.functions = functions.into_iter().collect();
    m
}

fn build(ids: &[&str], sources: &[&str], options: &Options) -> (Vec<u8>, Manifest) {
    let models: Vec<Message<'_>> = sources.iter().map(|s| parse(s)).collect();
    let manifest = manifest_of(ids, &models);
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
    (catalog(&manifest, &refs, options).unwrap(), manifest)
}

const IDS: [&str; 3] = ["a.greeting", "b.count", "c.link"];
const EN: [&str; 3] = [
    "Hello {$name}",
    ".input {$n :integer} .match $n 1 {{one}} * {{{$n} items}}",
    "Read {#link}this{/link}",
];

#[test]
fn f6_another_manifest_is_rejected() {
    let opts = Options::new("en", Dir::Ltr).stripped();
    let (bytes, manifest) = build(&IDS, &EN, &opts);
    let h = manifest.hash();
    assert!(Catalog::new(bytes.clone(), h).is_ok());
    // Each thing the wasm is compiled against changes the hash: an id, a
    // slot, a markup name, the function set.
    let variants: [[&str; 3]; 3] = [
        ["Hello {$user}", EN[1], EN[2]],
        [EN[0], EN[1], "Read {#a}this{/a}"],
        [
            EN[0],
            ".input {$n :number} .match $n 1 {{one}} * {{{$n} items}}",
            EN[2],
        ],
    ];
    for v in &variants {
        let (_, other) = build(&IDS, v, &opts);
        assert_ne!(other.hash(), h, "{v:?}");
        assert_eq!(
            Catalog::new(bytes.clone(), other.hash()).map(|_| ()),
            Err(CatalogError::ManifestMismatch)
        );
    }
    let (_, other) = build(&["a.greeting", "b.count", "c.linked"], &EN, &opts);
    assert_ne!(other.hash(), h);
}

#[test]
fn f6_a_translation_edit_keeps_the_hash() {
    // A translation that changes text, selectors and variants, and uses a
    // subset of the variables, is compiled against the same manifest.
    let opts = Options::new("pl", Dir::Ltr).stripped();
    let (_, source) = build(&IDS, &EN, &opts);
    let pl = [
        "Cześć",
        ".input {$n :integer} .match $n 1 {{jeden}} few {{{$n} elementy}} * {{{$n} elementów}}",
        "Czytaj {#link}to{/link}",
    ];
    let models: Vec<Message<'_>> = pl.iter().map(|s| parse(s)).collect();
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
    let bytes = catalog(&source, &refs, &opts).unwrap();
    let cat = Catalog::new(bytes, source.hash()).unwrap();
    for (i, m) in models.iter().enumerate() {
        assert_eq!(
            &decode(&cat, MsgId::new(0, u32::try_from(i).unwrap()).unwrap()).unwrap(),
            m
        );
    }
}

#[test]
fn f7_fallback_locales_round_trip() {
    let n = 300u32;
    let ids: Vec<String> = (0..n).map(|i| format!("id{i:04}")).collect();
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let sources: Vec<String> = (0..n).map(|i| format!("text {i}")).collect();
    let src_refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    let mut opts = Options::new("es-MX", Dir::Ltr);
    // Every third message from "es", every seventh from "en" (overriding).
    let mut expected = BTreeMap::new();
    for i in (0..n).step_by(3) {
        expected.insert(i, "es");
    }
    for i in (0..n).step_by(7) {
        expected.insert(i, "en");
    }
    opts.fallback = expected
        .iter()
        .rev()
        .map(|(&i, &t)| (i, t.to_owned()))
        .collect();
    let (bytes, manifest) = build(&id_refs, &src_refs, &opts);
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    for i in 0..n {
        let id = MsgId::new(0, i).unwrap();
        assert_eq!(cat.fallback_locale(id), expected.get(&i).copied(), "{i}");
    }
    assert_eq!(cat.fallback_locale(MsgId::new(1, 0).unwrap()), None);
    assert_eq!(cat.fallback_locale(MsgId::new(0, n).unwrap()), None);
    // Without fallbacks there is no FALLBACK section.
    let (bytes, manifest) = build(&id_refs, &src_refs, &Options::new("es", Dir::Ltr));
    let cat = Catalog::new(bytes, manifest.hash()).unwrap();
    assert!(
        cat.sections()
            .all(|(k, _, _)| k != mf2_catalog::format::section::FALLBACK)
    );
    assert_eq!(cat.fallback_locale(MsgId::new(0, 0).unwrap()), None);
}

#[test]
fn f9_unknown_major_versions_are_rejected() {
    let (bytes, manifest) = build(&IDS, &EN, &Options::new("en", Dir::Rtl));
    let h = manifest.hash();
    for major in [0u8, 2, 0xff] {
        let mut b = bytes.clone();
        b[5] = major;
        assert_eq!(Catalog::new(b, h).map(|_| ()), Err(CatalogError::Version));
    }
    let mut b = bytes;
    b[4] = 0x42; // a later minor: additive, still readable
    let cat = Catalog::new(b, h).unwrap();
    assert_eq!(cat.format_version(), 0x0142);
    assert_eq!(cat.dir(), Dir::Rtl);
}

#[test]
fn stripping_keeps_the_formatting_model_of_the_workload() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("bench/corpora/workload-1600.json")).unwrap();
    let corpus: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
    let ids: Vec<&str> = corpus.keys().map(String::as_str).collect();
    let sources: Vec<&str> = corpus.values().map(String::as_str).collect();
    let (full, manifest) = build(&ids, &sources, &Options::new("en", Dir::Ltr));
    let (stripped, _) = build(&ids, &sources, &Options::new("en", Dir::Ltr).stripped());
    assert!(stripped.len() < full.len());
    let full = Catalog::new(full, manifest.hash()).unwrap();
    let stripped = Catalog::new(stripped, manifest.hash()).unwrap();
    assert!(stripped.cold_stripped() && stripped.ids_stripped());
    assert!(!full.cold_stripped() && !full.ids_stripped());
    assert_eq!(stripped.lookup(ids[0]), None);
    for i in 0..u32::try_from(ids.len()).unwrap() {
        let id = MsgId::new(0, i).unwrap();
        let a = decode_report(&full, id).unwrap();
        let b = decode_report(&stripped, id).unwrap();
        assert!(!a.cold_dropped && !b.cold_dropped);
        assert_eq!(a.message, b.message);
    }
}
