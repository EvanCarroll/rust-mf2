//! `cargo xtask fuzz-seed`: writes the seed corpora of the fuzz targets
//! (git-ignored):
//!
//! * `fuzz/corpus/parse/` — every `src` of the vendored WG suite and every
//!   message of the committed reference workload, one file each;
//! * `fuzz/corpus/catalog/` — for every suite message that parses, its
//!   one-message catalog (`writer::single`, slots from `analyze`), unstripped
//!   and stripped, and its source — plain, and with an options byte and
//!   mutation instructions after a NUL (the target's source mode);
//!   and the reference workload as one catalog, its manifest built the way
//!   `mf2-build` will — unstripped with a plural entry and fallbacks (every
//!   section), and stripped;
//! * `fuzz/corpus/format/` — the `format` target's input (`[flags] [n]
//!   [n argument bytes] [payload]`): every L4 case of the suite, its catalog
//!   (for its locale, unstripped and stripped) and its source (source mode),
//!   with its `params` as positional arguments; the workload catalogs with a
//!   few arguments; and 400 generated L4 cases (`mf2_conformance::l4gen`);
//! * `fuzz/corpus/resource/` and `fuzz/corpus/pipeline/` — the reference
//!   workload and the suite's messages, each written as one resource file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{CldrVersion, Dir, Manifest};
use mf2_conformance::abnf::Grammar;
use mf2_conformance::spec::{ABNF, read_spec};
use mf2_conformance::{TestKind, l4gen};
use mf2_l4_runner::{ArgSpec, Case};
use mf2_runtime::BidiStrategy;

use crate::error::{Error, Result};

/// `en`'s `plural.cardinal` entry (`plans/02-catalog-format.md` §4.1).
const EN_CARDINAL: [u8; 5] = [0x21, 0x01, 0x05, 0x82, 0x01];

pub(crate) fn run(root: &Path) -> Result<()> {
    let suite = mf2_conformance::load_suite(root)?;
    let workload_path = root.join("bench/corpora/workload-1600.json");
    let text = fs::read_to_string(&workload_path).map_err(|source| Error::IoAt {
        path: workload_path.clone(),
        source,
    })?;
    let workload: BTreeMap<String, String> =
        serde_json::from_str(&text).map_err(|e| Error::Json {
            path: workload_path.clone(),
            message: e.to_string(),
        })?;

    let dir = create(root, "fuzz/corpus/parse")?;
    let sources = suite
        .tests()
        .iter()
        .map(|t| t.src.as_str())
        .chain(workload.values().map(String::as_str));
    let mut n = 0usize;
    for (i, src) in sources.enumerate() {
        write(&dir, &format!("seed-{i:05}"), src.as_bytes())?;
        n += 1;
    }
    eprintln!("fuzz-seed: wrote {n} files to {}", dir.display());

    let dir = create(root, "fuzz/corpus/catalog")?;
    let mut n = 0usize;
    for (i, t) in suite.tests().iter().enumerate() {
        let Some(model) = mf2_syntax::parse_model(&t.src).message else {
            continue;
        };
        let analysis = mf2_syntax::analyze(&model);
        let slots: Vec<&str> = analysis.externals.iter().map(|s| &*s.nfc).collect();
        let options = Options::new("en", Dir::Ltr);
        let (bytes, _) = writer::single(&model, &slots, &options)?;
        write(&dir, &format!("suite-{i:04}.mf2b"), &bytes)?;
        let (bytes, _) = writer::single(&model, &slots, &options.stripped())?;
        write(&dir, &format!("suite-{i:04}-stripped.mf2b"), &bytes)?;
        write(&dir, &format!("suite-{i:04}.mf2"), t.src.as_bytes())?;
        write(
            &dir,
            &format!("suite-{i:04}-damaged.mf2"),
            &damaged(&t.src, i),
        )?;
        n += 4;
    }
    let (full, stripped) = workload_catalogs(&workload)?;
    write(&dir, "workload.mf2b", &full)?;
    write(&dir, "workload-stripped.mf2b", &stripped)?;
    n += 2;
    eprintln!(
        "fuzz-seed: wrote {n} files to {} (workload catalog: {} B, stripped {} B)",
        dir.display(),
        full.len(),
        stripped.len()
    );

    let dir = create(root, "fuzz/corpus/format")?;
    let mut n = 0usize;
    for (i, t) in suite.tests().iter().enumerate() {
        if t.kind != TestKind::Other {
            continue;
        }
        let (unstripped, stripped) = mf2_conformance::l4::cases(t).map_err(Error::L4)?;
        let slots = slot_names(&t.src);
        let head = format_head(&unstripped, &slots);
        write(
            &dir,
            &format!("suite-{i:04}.bin"),
            &[&head[..], &unstripped.catalog].concat(),
        )?;
        write(
            &dir,
            &format!("suite-{i:04}-stripped.bin"),
            &[&head[..], &stripped.catalog].concat(),
        )?;
        let locale = FORMAT_LOCALES
            .iter()
            .position(|l| *l == t.locale)
            .unwrap_or(0);
        let locale = u8::try_from(locale).unwrap_or(0);
        write(
            &dir,
            &format!("suite-{i:04}-source.bin"),
            &[&head[..], t.src.as_bytes(), &[0, locale, 0]].concat(),
        )?;
        // The same in the default configuration (flags bit 1: layer L4d).
        let mut default_head = head.clone();
        if let Some(flags) = default_head.first_mut() {
            *flags |= 2;
        }
        write(
            &dir,
            &format!("suite-{i:04}-default.bin"),
            &[&default_head[..], t.src.as_bytes(), &[0, locale, 0]].concat(),
        )?;
        n += 4;
    }
    // A few arguments of every kind for the workload's first slots: unset,
    // a string, an i64, a decimal, an f64, an opaque value, a date/time
    // literal and an epoch instant.
    let mut args = vec![
        7, 0, 3, b'a', b'b', b'c', 1, 42, 0, 0, 0, 0, 0, 0, 0, 3, 4, b'1', b'.', b'5', b'0', 2, 0,
        0, 0, 0, 0, 0, 0xf8, 0x3f, 4, 5, 20,
    ];
    args.extend_from_slice(b"2006-01-02T15:04:06Z");
    args.push(6);
    args.extend_from_slice(&1_136_214_246_000_i64.to_le_bytes());
    for (name, catalog) in [("workload", &full), ("workload-stripped", &stripped)] {
        let len = u8::try_from(args.len()).unwrap_or(0);
        write(
            &dir,
            &format!("{name}.bin"),
            &[&[0, len][..], &args, catalog].concat(),
        )?;
        n += 1;
    }
    let text = read_spec(root, ABNF)?;
    let grammar = Grammar::parse(&text).map_err(|e| Error::L4(e.to_string()))?;
    for i in 0..400u64 {
        let g = l4gen::case(&grammar, 0x6d66_3274_776f + i).map_err(Error::L4)?;
        let head = format_head(&g.unstripped, &slot_names(&g.source));
        write(
            &dir,
            &format!("gen-{i:03}.bin"),
            &[&head[..], &g.unstripped.catalog].concat(),
        )?;
        n += 1;
    }
    eprintln!("fuzz-seed: wrote {n} files to {}", dir.display());

    let dir = create(root, "fuzz/corpus/resource")?;
    let mut n = 0usize;
    write(
        &dir,
        "workload.mf2",
        resource_file("en", &workload).as_bytes(),
    )?;
    n += 1;
    // The suite's messages as one resource: MF2 escapes, every placeholder
    // shape and the malformed sources, inside a container.
    let suite_entries: BTreeMap<String, String> = suite
        .tests()
        .iter()
        .enumerate()
        .map(|(i, t)| (format!("suite.t{i:04}"), t.src.clone()))
        .collect();
    write(
        &dir,
        "suite.mf2",
        resource_file("en", &suite_entries).as_bytes(),
    )?;
    n += 1;
    eprintln!("fuzz-seed: wrote {n} files to {}", dir.display());
    Ok(())
}

/// The entries as one resource file, sectioned by the first part of each id.
///
/// A source the container cannot write — a lone `\`, which only a malformed
/// message has — is left out.
fn resource_file(locale: &str, entries: &BTreeMap<String, String>) -> String {
    use mf2_resource::{Comment, Entry, Head, Id, Meta, Resource, Section, Span, Style, ValueMap};
    const NO_SPAN: Span = Span { start: 0, end: 0 };

    let mut resource = Resource::<&str>::default();
    resource.meta.push(Meta {
        name: "locale".into(),
        value: Some(locale.into()),
        span: NO_SPAN,
        value_span: None,
    });
    resource.comment = Some(Comment {
        text: "Seed for the `resource` fuzz target (cargo xtask fuzz-seed).".into(),
        span: NO_SPAN,
    });
    for (id, source) in entries {
        let Ok(full) = Id::read(id) else { continue };
        let (head, own) = match full.parts().split_first() {
            Some((first, rest)) if !rest.is_empty() => {
                (Id::new(vec![first.clone()]), Id::new(rest.to_vec()))
            }
            _ => (Id::default(), full.clone()),
        };
        let section = match resource.sections.last() {
            Some(last) if last.head.as_ref().map(|h| &h.id) == Some(&head) => {
                resource.sections.last_mut().expect("just matched")
            }
            _ => {
                resource.sections.push(Section {
                    head: (!head.is_empty()).then(|| Head {
                        id: head,
                        comment: None,
                        meta: Vec::new(),
                        span: NO_SPAN,
                    }),
                    entries: Vec::new(),
                    detached: Vec::new(),
                });
                resource.sections.last_mut().expect("just pushed")
            }
        };
        section.entries.push(Entry {
            id: own,
            value: source.as_str(),
            comment: None,
            meta: Vec::new(),
            span: NO_SPAN,
            id_span: NO_SPAN,
            value_span: NO_SPAN,
            map: ValueMap::Empty,
        });
    }
    // Drop what the container cannot write, then write the rest.
    for section in &mut resource.sections {
        section
            .entries
            .retain(|e| mf2_resource::serialize(&one_entry(e)).is_ok());
    }
    resource.sections.retain(|s| !s.entries.is_empty());
    mf2_resource::serialize_with(&resource, &Style::wrapped(100, 76))
        .expect("every unwritable entry was dropped")
}

/// One entry on its own, to test whether it can be written.
fn one_entry<'a>(entry: &mf2_resource::Entry<'a, &'a str>) -> mf2_resource::Resource<'a, &'a str> {
    mf2_resource::Resource {
        comment: None,
        meta: Vec::new(),
        sections: vec![mf2_resource::Section {
            head: None,
            entries: vec![entry.clone()],
            detached: Vec::new(),
        }],
    }
}

/// The `format` target's source-mode locales (`fuzz/fuzz_targets/format.rs`).
const FORMAT_LOCALES: [&str; 17] = [
    "en",
    "pl",
    "ar",
    "he",
    "cy",
    "ja",
    "fr-CA",
    "und",
    "es",
    "de",
    "fr",
    "hi",
    "ru",
    "ar-EG",
    "hi-u-nu-deva",
    "en-US",
    "sr-Latn",
];

/// The slot (external variable) names of `src`, in slot order.
fn slot_names(src: &str) -> Vec<String> {
    mf2_syntax::parse_model(src)
        .message
        .map_or_else(Vec::new, |m| {
            mf2_syntax::analyze(&m)
                .externals
                .iter()
                .map(|n| n.nfc.to_string())
                .collect()
        })
}

/// The `format` target's head for `case`: `[flags] [n] [n argument bytes]`,
/// its named arguments put in slot order (unset where a slot has none).
fn format_head(case: &Case, slots: &[String]) -> Vec<u8> {
    let mut args = Vec::new();
    for slot in slots {
        let arg = case.args.iter().find(|(n, _)| n == slot).map(|(_, a)| a);
        let mut one = Vec::new();
        match arg {
            Some(ArgSpec::Str(s)) => {
                one.push(0);
                push_text(&mut one, s);
            }
            Some(ArgSpec::Int(v)) => {
                one.push(1);
                one.extend_from_slice(&v.to_le_bytes());
            }
            Some(ArgSpec::Float(x)) => {
                one.push(2);
                one.extend_from_slice(&x.to_bits().to_le_bytes());
            }
            Some(ArgSpec::Decimal(d)) => {
                one.push(3);
                push_text(&mut one, d);
            }
            Some(ArgSpec::Other) => one.push(4),
            // A date/time argument, as its literal text.
            Some(ArgSpec::DateTime(d)) => {
                one.push(5);
                let mut iso = String::new();
                d.write_iso(&mut iso);
                push_text(&mut one, &iso);
            }
            None => one.push(7),
        }
        if args.len() + one.len() > usize::from(u8::MAX) {
            break;
        }
        args.extend_from_slice(&one);
    }
    let flags = u8::from(case.bidi == BidiStrategy::None)
        | u8::from(case.config == mf2_l4_runner::Config::Default) << 1;
    let mut head = vec![flags, u8::try_from(args.len()).unwrap_or(0)];
    head.extend_from_slice(&args);
    head
}

/// `[len] bytes`, cut to 255 bytes on a char boundary.
fn push_text(out: &mut Vec<u8>, s: &str) {
    let mut end = s.len().min(255);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out.push(u8::try_from(end).unwrap_or(0));
    out.extend_from_slice(&s.as_bytes()[..end]);
}

/// The reference workload as one catalog (ids ascending; slots and markup
/// from `analyze`; the functions the messages use), unstripped — with `en`'s
/// plural rule and every 97th message from a fallback locale, so that every
/// section is present — and stripped.
fn workload_catalogs(workload: &BTreeMap<String, String>) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut models = Vec::with_capacity(workload.len());
    let mut manifest = Manifest::default();
    let mut functions = BTreeSet::new();
    for (id, src) in workload {
        let Some(model) = mf2_syntax::parse_model(src).message else {
            return Err(Error::Json {
                path: PathBuf::from("bench/corpora/workload-1600.json"),
                message: format!("message {id:?} does not parse"),
            });
        };
        let a = mf2_syntax::analyze(&model);
        manifest.ids.push(id.clone());
        manifest
            .slots
            .push(a.externals.iter().map(|s| s.nfc.to_string()).collect());
        manifest
            .markup
            .push(a.markup.iter().map(|s| s.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|s| s.nfc.to_string()));
        models.push(model);
    }
    manifest.functions = functions.into_iter().collect();
    let refs: Vec<Option<&_>> = models.iter().map(Some).collect();
    let mut options = Options::new("en", Dir::Ltr);
    options.cldr_version = Some(CldrVersion {
        major: 48,
        minor: 2,
        patch: 1,
    });
    options.locale_entries = vec![(1, EN_CARDINAL.to_vec())];
    let stripped = writer::catalog(&manifest, &refs, &options.clone().stripped())?;
    options.fallback = (0..refs.len())
        .step_by(97)
        .map(|i| (u32::try_from(i).unwrap_or(0), String::from("de")))
        .collect();
    let full = writer::catalog(&manifest, &refs, &options)?;
    Ok((full, stripped))
}

/// A source-mode input of the `catalog` target: `src`, NUL, an options byte
/// and four mutation instructions (`[op, pos_lo, pos_hi, val]`), all derived
/// from `seed` so that the corpus is reproducible.
fn damaged(src: &str, seed: usize) -> Vec<u8> {
    let mut x = u64::try_from(seed)
        .unwrap_or(0)
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut out = Vec::with_capacity(src.len() + 18);
    out.extend_from_slice(src.as_bytes());
    out.push(0);
    for _ in 0..17 {
        // xorshift64*
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        out.push(x.wrapping_mul(0x2545_f491_4f6c_dd1d).to_le_bytes()[7]);
    }
    out
}

fn create(root: &Path, rel: &str) -> Result<PathBuf> {
    let dir = root.join(rel);
    fs::create_dir_all(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = dir.join(name);
    fs::write(&path, bytes).map_err(|source| Error::IoAt { path, source })
}
