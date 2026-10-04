//! Linear time on adversarial catalogs (A7):
//! `Catalog::new` walks each section once; the
//! views, the decoder and `lookup` are linear in the bytes they touch; the
//! writer's deduplication is sorted.
//!
//! Every case is built twice, at a scale `n` and at `4n` (the larger one
//! 100–300 KB; 650 KB for 65,534 section-table entries) — with the writer
//! where it can express the shape, by hand
//! where it cannot — and then loaded, walked completely (every declaration,
//! part, expression, option, markup, selector, variant and key; `text` on
//! every string; every variable through NAMES and every function through
//! FUNCS), decoded message by message and looked up by id. Two bounds hold:
//! an absolute one, generous enough for a debug build on a loaded machine,
//! and the growth from `n` to `4n`, which is 4× for a linear implementation
//! and 16× for a quadratic one; the test allows 8×. Each case also checks
//! that the walk really visited what it was built to contain, so a case
//! cannot pass by being rejected or cut short. Writing is timed too (an
//! absolute bound; the growth is printed with `--nocapture`).
//!
//! In a debug build on a quiet machine the whole test takes about 4 s; the
//! growth of every case is 3.6–4.6×. One superlinear shape is known and open:
//! see [`records_that_run_into_the_next_one`].

use std::borrow::Cow;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use mf2_catalog::format::{HEADER_LEN, MAGIC, SECTION_ENTRY_LEN, VERSION, kind, section};
use mf2_catalog::writer::{Options, catalog, single};
use mf2_catalog::{
    Body, Catalog, DeclView, Dir, Entry, ExprView, KeyView, Manifest, MsgId, Names, Operand,
    OptionsView, PartView, PatternView, StrRef, VarRef, decode,
};
use mf2_model::{
    CatchAllKey, Declaration, Key, Message, Pattern, PatternPart, SelectMessage, VariableRef,
    Variant,
};

/// Timed runs per catalog; the fastest counts (the machine is shared).
const RUNS: usize = 3;
/// Absolute bound for loading, walking, decoding and looking up one catalog.
const LIMIT: Duration = Duration::from_secs(5);
/// Absolute bound for writing one catalog.
const WRITE_LIMIT: Duration = Duration::from_secs(20);
/// Largest allowed `t(4n) / t(n)`: 4 is linear, 16 quadratic.
const GROWTH: f64 = 8.0;
/// Below this, `t(4n)` is too small to compare (timer and scheduler noise).
const NOISE: Duration = Duration::from_millis(20);
/// The manifest hash of the hand-built catalogs.
const RAW_HASH: u64 = 0x5eed_0000_0000_0a07;

// ── what a walk visits ───────────────────────────────────────────────────────

/// Counts of everything a full walk visits.
#[derive(Clone, Copy, Default, Debug)]
struct Stats {
    sections: usize,
    messages: usize,
    decls: usize,
    parts: usize,
    options: usize,
    selectors: usize,
    variants: usize,
    keys: usize,
    vars: usize,
    functions: usize,
    strings: usize,
    text_bytes: usize,
    locale_bytes: usize,
    fallbacks: usize,
    decoded: usize,
    attributes: usize,
    found: usize,
    malformed: usize,
}

impl Stats {
    fn fields(&self) -> [(&'static str, usize); 18] {
        [
            ("sections", self.sections),
            ("messages", self.messages),
            ("decls", self.decls),
            ("parts", self.parts),
            ("options", self.options),
            ("selectors", self.selectors),
            ("variants", self.variants),
            ("keys", self.keys),
            ("vars", self.vars),
            ("functions", self.functions),
            ("strings", self.strings),
            ("text_bytes", self.text_bytes),
            ("locale_bytes", self.locale_bytes),
            ("fallbacks", self.fallbacks),
            ("decoded", self.decoded),
            ("attributes", self.attributes),
            ("found", self.found),
            ("malformed", self.malformed),
        ]
    }

    /// The first field below its minimum in `want`.
    fn short_of(&self, want: &Stats) -> Option<(&'static str, usize, usize)> {
        self.fields()
            .into_iter()
            .zip(want.fields())
            .find(|((_, got), (_, min))| got < min)
            .map(|((name, got), (_, min))| (name, got, min))
    }
}

/// A full walk of a loaded catalog.
struct Walk<'c> {
    cat: &'c Catalog,
    names: Names<'c>,
    s: Stats,
}

impl<'c> Walk<'c> {
    fn text(&mut self, r: StrRef) {
        if let Some(t) = self.cat.text(r) {
            self.s.strings += 1;
            self.s.text_bytes += t.len();
        }
    }

    fn var(&mut self, v: VarRef) {
        self.s.vars += 1;
        if let Some(r) = self.names.var(v) {
            self.text(r);
        }
    }

    fn operand(&mut self, o: Operand) {
        match o {
            Operand::Literal(r) => self.text(r),
            Operand::Variable(v) => self.var(v),
        }
    }

    fn options(&mut self, o: OptionsView<'c>) {
        for opt in o {
            let Ok((name, value)) = opt else {
                self.s.malformed += 1;
                break;
            };
            self.s.options += 1;
            self.text(name);
            self.operand(value);
        }
    }

    fn expr(&mut self, e: ExprView<'c>) {
        if let Some(o) = e.operand() {
            self.operand(o);
        }
        if let Some(f) = e.function() {
            self.s.functions += 1;
            if let Some(name) = self.cat.function(f.index()) {
                self.s.text_bytes += name.len();
            }
            self.options(f.options());
        }
    }

    fn pattern(&mut self, p: PatternView<'c>) {
        for part in p.parts() {
            let Ok(part) = part else {
                self.s.malformed += 1;
                break;
            };
            self.s.parts += 1;
            match part {
                PartView::Text(r) => self.text(r),
                PartView::Expression(e) => self.expr(e),
                PartView::Markup(m) => {
                    self.text(m.name());
                    self.options(m.options());
                }
            }
        }
    }

    fn message(&mut self, id: MsgId) {
        if self.cat.fallback_locale(id).is_some() {
            self.s.fallbacks += 1;
        }
        match self.cat.get(id) {
            Entry::Absent => return,
            Entry::Simple(r) => {
                self.s.messages += 1;
                self.text(r);
            }
            Entry::Pattern(v) | Entry::Select(v) => {
                self.s.messages += 1;
                self.names = v.names();
                let mut decls = v.declarations();
                for d in &mut decls {
                    let Ok(DeclView::Input(e) | DeclView::Local { expr: e, .. }) = d else {
                        self.s.malformed += 1;
                        break;
                    };
                    self.s.decls += 1;
                    self.expr(e);
                }
                match decls.body() {
                    Ok(Body::Pattern(p)) => self.pattern(p),
                    Ok(Body::Select(s)) => {
                        for sel in s.selectors() {
                            let Ok(v) = sel else {
                                self.s.malformed += 1;
                                break;
                            };
                            self.s.selectors += 1;
                            self.var(v);
                        }
                        for variant in s.variants() {
                            let Ok(variant) = variant else {
                                self.s.malformed += 1;
                                break;
                            };
                            self.s.variants += 1;
                            for k in variant.keys() {
                                match k {
                                    Ok(KeyView::Literal(r)) => {
                                        self.s.keys += 1;
                                        self.text(r);
                                    }
                                    Ok(KeyView::CatchAll) => self.s.keys += 1,
                                    Err(_) => {
                                        self.s.malformed += 1;
                                        break;
                                    }
                                }
                            }
                            self.pattern(variant.pattern());
                        }
                    }
                    Err(_) => self.s.malformed += 1,
                }
            }
        }
        if let Ok(m) = decode(self.cat, id) {
            self.s.decoded += 1;
            self.s.attributes += attributes(&m);
        }
    }
}

/// The attributes of a decoded message (they live in COLD).
fn attributes(m: &Message<'_>) -> usize {
    let mut n = 0;
    for d in m.declarations() {
        n += match d {
            Declaration::Input(x) => x.value.attributes.len(),
            Declaration::Local(x) => x.value.attributes().len(),
            _ => 0,
        };
    }
    let patterns: Vec<&Pattern<'_>> = match m {
        Message::Pattern(p) => vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
        _ => Vec::new(),
    };
    for p in patterns {
        for part in p.parts() {
            n += match part {
                PatternPart::Expression(e) => e.attributes().len(),
                PatternPart::Markup(m) => m.attributes.len(),
                _ => 0,
            };
        }
    }
    n
}

/// Loads, walks, decodes and looks up once; returns what it saw and how
/// long it took.
fn run_once(bytes: &[u8], hash: u64, lookups: &[String]) -> (Stats, Duration) {
    let owned = bytes.to_vec();
    let start = Instant::now();
    let cat = Catalog::new(owned, hash).expect("the catalog loads");
    let mut w = Walk {
        cat: &cat,
        names: Names::EMPTY,
        s: Stats::default(),
    };
    w.s.sections = cat.sections().count();
    let _ = (cat.locale(), cat.dir(), cat.cldr_version());
    // 64: the first opaque key of `locale_entries` (keys 3 and 4 are
    // `number.*`, which the writer checks; 64 and up are unassigned).
    for key in [0, 1, 2, 64, u32::MAX] {
        w.s.locale_bytes += cat.locale_entry(key).map_or(0, <[u8]>::len);
    }
    for i in 0..cat.message_count() {
        w.message(MsgId::new(cat.chunk(), i).expect("a valid id"));
    }
    for id in lookups {
        if cat.lookup(id).is_some() {
            w.s.found += 1;
        }
    }
    (w.s, start.elapsed())
}

/// The fastest of [`RUNS`] runs.
fn run(bytes: &[u8], hash: u64, lookups: &[String]) -> (Stats, Duration) {
    let (stats, mut best) = run_once(bytes, hash, lookups);
    for _ in 1..RUNS {
        best = best.min(run_once(bytes, hash, lookups).1);
    }
    (stats, best)
}

// ── building catalogs ────────────────────────────────────────────────────────

/// A catalog to time, what its walk must at least visit, and how long
/// writing it took (zero for hand-built ones).
struct Case {
    bytes: Vec<u8>,
    hash: u64,
    lookups: Vec<String>,
    want: Stats,
    write: Duration,
}

impl Case {
    fn new(bytes: Vec<u8>, hash: u64, want: Stats) -> Case {
        Case {
            bytes,
            hash,
            lookups: Vec::new(),
            want,
            write: Duration::ZERO,
        }
    }
}

fn parse(src: &str) -> Message<'_> {
    let p = mf2_syntax::parse_model(src);
    p.message
        .unwrap_or_else(|| panic!("does not parse: {:?}", p.diagnostics))
}

/// One message through `writer::single`, timed.
fn single_case(m: &Message<'_>, want: Stats) -> Case {
    let a = mf2_syntax::analyze(m);
    let slots: Vec<&str> = a.externals.iter().map(|n| &*n.nfc).collect();
    let start = Instant::now();
    let (bytes, manifest) = single(m, &slots, &Options::new("en", Dir::Ltr)).expect("writes");
    let write = start.elapsed();
    Case {
        write,
        ..Case::new(bytes, manifest.hash(), want)
    }
}

fn source_case(src: &str, want: Stats) -> Case {
    single_case(&parse(src), want)
}

/// Many messages through `writer::catalog` (ids ascending), timed; every id
/// is looked up.
fn catalog_case(ids: Vec<String>, sources: &[String], options: &Options, want: Stats) -> Case {
    let models: Vec<Message<'_>> = sources.iter().map(|s| parse(s)).collect();
    let mut manifest = Manifest::default();
    let mut functions = std::collections::BTreeSet::new();
    for m in &models {
        let a = mf2_syntax::analyze(m);
        manifest
            .slots
            .push(a.externals.iter().map(|n| n.nfc.to_string()).collect());
        manifest
            .markup
            .push(a.markup.iter().map(|n| n.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|n| n.nfc.to_string()));
    }
    manifest.ids = ids;
    manifest.functions = functions.into_iter().collect();
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
    let start = Instant::now();
    let bytes = catalog(&manifest, &refs, options).expect("writes");
    let write = start.elapsed();
    Case {
        lookups: manifest.ids.clone(),
        write,
        ..Case::new(bytes, manifest.hash(), want)
    }
}

fn varint(mut v: u32, out: &mut Vec<u8>) {
    loop {
        let low = u8::try_from(v & 0x7f).expect("7 bits");
        v >>= 7;
        if v == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).expect("fits u32")
}

/// INDEX as four byte planes.
fn planes(entries: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 4);
    for plane in 0..4 {
        out.extend(entries.iter().map(|e| e.to_le_bytes()[plane]));
    }
    out
}

/// A hand-built catalog: header, section
/// table, sections in the order given (STRINGS must be last), manifest hash
/// [`RAW_HASH`], locale at `StrRef` 0.
fn raw(count: usize, sections: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let table_end = HEADER_LEN + sections.len() * SECTION_ENTRY_LEN;
    let mut out = Vec::with_capacity(table_end + sections.iter().map(|s| s.1.len()).sum::<usize>());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&RAW_HASH.to_le_bytes());
    out.extend_from_slice(&u32_of(count).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // locale: StrRef 0
    out.extend_from_slice(&0u32.to_le_bytes()); // no CLDR data
    out.extend_from_slice(&[0, 0]); // chunk 0, ltr
    out.extend_from_slice(
        &u16::try_from(sections.len())
            .expect("≤ 65,535")
            .to_le_bytes(),
    );
    let mut off = table_end;
    for (k, data) in sections {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&u32_of(off).to_le_bytes());
        out.extend_from_slice(&u32_of(data.len()).to_le_bytes());
        off += data.len();
    }
    for (_, data) in sections {
        out.extend_from_slice(data);
    }
    out
}

/// The required sections of a hand-built catalog around `index`,
/// `messages`, `names` and `pool` (empty LOCALE and FUNCS).
fn required(
    index: Vec<u8>,
    messages: Vec<u8>,
    names: Vec<u8>,
    pool: Vec<u8>,
) -> Vec<(u16, Vec<u8>)> {
    vec![
        (section::INDEX, index),
        (section::MESSAGES, messages),
        (section::NAMES, names),
        (section::LOCALE, vec![0]),
        (section::FUNCS, Vec::new()),
        (section::STRINGS, pool),
    ]
}

// ── the cases ────────────────────────────────────────────────────────────────

fn function_options(n: usize) -> Case {
    let src = format!("{{$x :f{}}}", " o=v".repeat(n));
    let want = Stats {
        options: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn distinct_option_names(n: usize) -> Case {
    let mut src = String::from("{$x :f");
    for i in 0..n {
        let _ = write!(src, " o{i}=v{i}");
    }
    src.push('}');
    let want = Stats {
        options: n,
        strings: 2 * n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn markup_options(n: usize) -> Case {
    let src = format!("{{#m{}}}x{{/m{}}}", " o=v".repeat(n), " o=$x".repeat(n));
    let want = Stats {
        options: 2 * n,
        vars: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn many_slots(n: usize) -> Case {
    let mut src = String::from("{:f");
    for i in 0..n {
        let _ = write!(src, " o=$v{i}");
    }
    src.push('}');
    let want = Stats {
        options: n,
        vars: n,
        strings: 2 * n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn variants(n: usize) -> Case {
    let mut src = String::from(".input {$x :f} .match $x");
    for i in 0..n {
        let _ = write!(src, " k{i} {{{{t}}}}");
    }
    src.push_str(" * {{t}}");
    let want = Stats {
        variants: n + 1,
        keys: n + 1,
        parts: n + 1,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn selectors_and_keys(n: usize) -> Case {
    let src = format!(
        ".input {{$x :f}} .match{} {}{{{{t}}}} {}{{{{u}}}}",
        " $x".repeat(n),
        "* ".repeat(n),
        "a ".repeat(n)
    );
    let want = Stats {
        selectors: n,
        variants: 2,
        keys: 2 * n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn chain_of_locals(n: usize) -> Case {
    let mut src = String::from(".input {$a0 :f}");
    for i in 1..n {
        let _ = write!(src, " .local $a{i} = {{$a{}}}", i - 1);
    }
    src.push_str(" {{");
    let last = format!("{{$a{}}}", n - 1);
    src.push_str(&last.repeat(n));
    for j in 0..n / 4 {
        let _ = write!(src, "{{$z{j}}}");
    }
    src.push_str("}}");
    let want = Stats {
        decls: n,
        vars: 2 * n + n / 4,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn many_parts(n: usize) -> Case {
    let src = "x{$y}".repeat(n);
    let want = Stats {
        parts: 2 * n,
        vars: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn many_functions(n: usize) -> Case {
    let mut src = String::new();
    for i in 0..n {
        let _ = write!(src, "{{:f{i}}}");
    }
    let want = Stats {
        functions: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn many_attributes(n: usize) -> Case {
    let src = format!("{{$x{}}}", " @a=b".repeat(n));
    let want = Stats {
        attributes: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn spelling_overrides(n: usize) -> Case {
    // `e` + U+0301 is not NFC: every option name gets a COLD override.
    let src = format!("{{:f{}}}", " e\u{301}=1".repeat(n));
    let want = Stats {
        options: n,
        decoded: 1,
        ..Stats::default()
    };
    source_case(&src, want)
}

fn catch_all_values(n: usize) -> Case {
    let values: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let m = Message::Select(SelectMessage {
        declarations: Vec::new(),
        selectors: vec![VariableRef {
            name: Cow::Borrowed("x"),
        }],
        variants: values
            .iter()
            .map(|v| Variant {
                keys: vec![Key::CatchAll(CatchAllKey {
                    value: Some(Cow::Borrowed(v)),
                })],
                value: Pattern::from_text(Cow::Borrowed("t")),
            })
            .collect(),
    });
    let want = Stats {
        variants: n,
        keys: n,
        decoded: 1,
        ..Stats::default()
    };
    single_case(&m, want)
}

fn many_messages(n: usize) -> Case {
    let ids = (0..n).map(|i| format!("m{i:06}")).collect();
    let sources: Vec<String> = (0..n)
        .map(|i| match i % 3 {
            0 => format!("Hello {i}"),
            1 => String::from("Hi {$name}!"),
            _ => String::from(".input {$n :integer} .match $n 1 {{one}} * {{{$n} items}}"),
        })
        .collect();
    let want = Stats {
        messages: n,
        decoded: n,
        found: n,
        ..Stats::default()
    };
    catalog_case(ids, &sources, &Options::new("en", Dir::Ltr), want)
}

fn long_ids(n: usize) -> Case {
    let prefix = "p".repeat(200);
    let ids = (0..n).map(|i| format!("{prefix}{i:06}")).collect();
    let sources = vec![String::from("x"); n];
    let want = Stats {
        messages: n,
        found: n,
        ..Stats::default()
    };
    let mut case = catalog_case(ids, &sources, &Options::new("en", Dir::Ltr), want);
    // Misses: before, between and after the ids.
    case.lookups.extend([
        String::new(),
        prefix.clone(),
        format!("{prefix}0000005"),
        format!("{prefix}z"),
    ]);
    case
}

fn one_long_id(n: usize) -> Case {
    let prefix = "a".repeat(4 * n);
    let ids = (0..20).map(|i| format!("{prefix}{i:02}")).collect();
    let sources = vec![String::from("x"); 20];
    let want = Stats {
        found: 20,
        ..Stats::default()
    };
    let mut case = catalog_case(ids, &sources, &Options::new("en", Dir::Ltr), want);
    case.lookups
        .extend([prefix.clone(), format!("{prefix}0"), format!("{prefix}99")]);
    case
}

fn fallbacks(n: usize) -> Case {
    let ids = (0..n).map(|i| format!("m{i:06}")).collect();
    let sources = vec![String::from("x"); n];
    let mut options = Options::new("en", Dir::Ltr).stripped();
    options.fallback = (0..n)
        .map(|i| (u32_of(i), format!("l{}", i % 256)))
        .collect();
    let want = Stats {
        fallbacks: n,
        ..Stats::default()
    };
    catalog_case(ids, &sources, &options, want)
}

fn locale_entries(n: usize) -> Case {
    // A long `plural.cardinal` rule: `one: i = 1 and i = 1 and …` (§4.1).
    let mut plural = vec![0x21];
    for _ in 1..n {
        plural.extend_from_slice(&[0x01, 0x05]);
    }
    plural.extend_from_slice(&[0x81, 0x05]);
    let mut options = Options::new("en", Dir::Ltr);
    options.locale_entries = vec![(1, plural)];
    options
        .locale_entries
        .extend((0..n).map(|i| (u32_of(i) + 64, vec![7])));
    let src = "{$n}";
    let m = parse(src);
    let manifest = Manifest {
        ids: vec![String::new()],
        slots: vec![vec![String::from("n")]],
        markup: vec![Vec::new()],
        functions: Vec::new(),
    };
    let start = Instant::now();
    let bytes = catalog(&manifest, &[Some(&m)], &options).expect("writes");
    let write = start.elapsed();
    let want = Stats {
        locale_bytes: 2 * n,
        decoded: 1,
        ..Stats::default()
    };
    Case {
        write,
        ..Case::new(bytes, manifest.hash(), want)
    }
}

/// `m` pattern messages sharing one NAMES entry of `k` slot names; each
/// refers to the last slot (by hand: a manifest of `m × k` names is not
/// needed to build it).
fn shared_names(n: usize) -> Case {
    let (k, m) = (2 * n, 2 * n);
    let pool = b"en\0x\0".to_vec();
    let mut names = Vec::new();
    varint(u32_of(k), &mut names);
    varint(0, &mut names);
    for _ in 0..k {
        names.extend_from_slice(&3u32.to_le_bytes());
    }
    let mut messages = Vec::new();
    let mut index = Vec::with_capacity(m);
    for _ in 0..m {
        index.push((kind::PATTERN << kind::SHIFT) | u32_of(messages.len()));
        // head: NAMES entry @0; no declarations; one part: `{$x}`, slot k − 1.
        messages.extend_from_slice(&[0x01, 0x00, 0x01, 0x11]);
        varint(u32_of(k - 1) << 1, &mut messages);
    }
    let bytes = raw(m, &required(planes(&index), messages, names, pool));
    let want = Stats {
        messages: m,
        vars: m,
        strings: m,
        decoded: m,
        ..Stats::default()
    };
    Case::new(bytes, RAW_HASH, want)
}

/// `n` empty sections of an unknown kind before the known ones.
fn many_sections(n: usize) -> Case {
    let mut sections = vec![(100u16, Vec::new()); n];
    let index = planes(&[(kind::SIMPLE << kind::SHIFT) | 3]);
    sections.extend(required(index, Vec::new(), Vec::new(), b"en\0x\0".to_vec()));
    let want = Stats {
        sections: n + 6,
        messages: 1,
        decoded: 1,
        ..Stats::default()
    };
    Case::new(raw(1, &sections), RAW_HASH, want)
}

type Build = fn(usize) -> Case;

/// `(name, n, build)`: `build(4 * n)` is the larger catalog.
const CASES: &[(&str, usize, Build)] = &[
    ("function options", 12_500, function_options),
    ("distinct option names", 3_000, distinct_option_names),
    ("markup options", 6_000, markup_options),
    ("variables in options (many slots)", 4_000, many_slots),
    ("variants", 4_000, variants),
    ("selectors and keys", 12_500, selectors_and_keys),
    ("chain of locals", 2_500, chain_of_locals),
    ("pattern parts", 12_500, many_parts),
    ("FUNCS entries", 4_000, many_functions),
    ("attributes (one COLD override)", 12_500, many_attributes),
    ("spelling overrides (COLD)", 12_500, spelling_overrides),
    ("catch-all values (COLD)", 3_000, catch_all_values),
    ("messages", 2_500, many_messages),
    ("ids with a long common prefix", 2_500, long_ids),
    ("one long id", 8_000, one_long_id),
    ("fallbacks", 5_000, fallbacks),
    (
        "LOCALE entries and a long plural rule",
        10_000,
        locale_entries,
    ),
    ("NAMES entry shared by many messages", 3_000, shared_names),
    ("unknown sections", 16_382, many_sections),
];

#[test]
fn adversarial_catalogs_in_linear_time() {
    let mut report = String::new();
    let mut failures = Vec::new();
    for &(what, n, build) in CASES {
        let mut times = [Duration::ZERO; 2];
        let mut line = format!("{what:40}");
        for (slot, scale) in [(0, n), (1, 4 * n)] {
            let case = build(scale);
            let (stats, t) = run(&case.bytes, case.hash, &case.lookups);
            if let Some((field, got, min)) = stats.short_of(&case.want) {
                failures.push(format!("{what} (n = {scale}): {field} = {got} < {min}"));
            }
            if stats.malformed != 0 {
                failures.push(format!(
                    "{what} (n = {scale}): {} malformed",
                    stats.malformed
                ));
            }
            if t > LIMIT {
                failures.push(format!("{what} (n = {scale}): {t:?} > {LIMIT:?}"));
            }
            if case.write > WRITE_LIMIT {
                failures.push(format!(
                    "{what} (n = {scale}): writing took {:?} > {WRITE_LIMIT:?}",
                    case.write
                ));
            }
            times[slot] = t;
            let _ = write!(
                line,
                " | {:>7} B {:>9.2?} (write {:>9.2?})",
                case.bytes.len(),
                t,
                case.write
            );
        }
        let growth = times[1].as_secs_f64() / times[0].as_secs_f64().max(1e-9);
        let _ = writeln!(line, " | ×{growth:.1}");
        report.push_str(&line);
        if times[1] >= NOISE && growth > GROWTH {
            failures.push(format!(
                "{what}: t(4n) / t(n) = {growth:.1} > {GROWTH} ({:?} → {:?})",
                times[0], times[1]
            ));
        }
    }
    eprint!("{report}");
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The section table at its limit: 65,535 entries (`section_count` is a
/// `u16`), all but the six required ones of unknown kinds.
#[test]
fn section_table_at_its_limit() {
    let case = many_sections(usize::from(u16::MAX) - 6);
    let start = Instant::now();
    let (stats, _) = run_once(&case.bytes, case.hash, &[]);
    let t = start.elapsed();
    assert_eq!(stats.sections, usize::from(u16::MAX));
    assert_eq!(stats.decoded, 1);
    assert!(t < LIMIT, "{t:?}");
}

/// `n` pattern messages of 8 bytes whose part count (65,536) runs past their
/// own record: its last byte and each following record read as more TEXT
/// parts (`00 00` · `00 00` · `00` + the next head, the next declaration
/// count + part count …), so every walk continues to the end of MESSAGES.
fn runaway_records(n: usize) -> Case {
    let parts: u32 = 1 << 16; // the part count, and a StrRef inside the pool
    let mut pool = b"en\0".to_vec();
    pool.resize(usize::try_from(parts).expect("fits"), b'a');
    pool.extend_from_slice(b"z\0");
    let mut messages = Vec::new();
    let mut index = Vec::with_capacity(n);
    for _ in 0..n {
        index.push((kind::PATTERN << kind::SHIFT) | u32_of(messages.len()));
        messages.extend_from_slice(&[0x00, 0x00]); // head: no NAMES, no COLD; no declarations
        varint(parts, &mut messages);
        messages.extend_from_slice(&[0x00, 0x00, 0x00]); // TEXT "en", then a TEXT tag
    }
    let bytes = raw(n, &required(planes(&index), messages, Vec::new(), pool));
    let want = Stats {
        messages: n,
        parts: n,
        ..Stats::default()
    };
    Case::new(bytes, RAW_HASH, want)
}

/// Records that run into the next one (the A7 finding). A MESSAGES record
/// carries no length, so a pattern's part count — likewise a declaration,
/// selector or variant count — can run on through the records after it;
/// walking *every* message of such a catalog is O(messages × MESSAGES)
/// (1,000 → 2,000 → 4,000 records: 0.6 → 2.5 → 9.7 s in a debug build). The
/// format accepts that: a
/// length per record would not bound strings, which F4 checks per access.
/// What it guarantees, and this test holds: **one** message's walk is linear
/// in the catalog, however far it runs.
#[test]
fn a_runaway_record_is_linear_in_the_catalog() {
    let walk_first = |case: &Case| {
        let cat = Catalog::new(case.bytes.clone(), RAW_HASH).expect("loads");
        let start = Instant::now();
        let Entry::Pattern(v) = cat.get(MsgId::from_raw(0)) else {
            panic!("not a pattern")
        };
        let Ok(Body::Pattern(p)) = v.body() else {
            panic!("no body")
        };
        let parts = p.parts().flatten().count();
        (parts, start.elapsed())
    };
    let n = 2_000;
    let (small, t_small) = walk_first(&runaway_records(n));
    let (large, t_large) = walk_first(&runaway_records(4 * n));
    // The first record's walk reads (nearly) every record after it.
    assert!(small > 2 * n && large > 8 * n, "{small} / {large} parts");
    let growth = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-9);
    assert!(
        t_large < NOISE || growth <= GROWTH,
        "t(4n) / t(n) = {growth:.1} ({t_small:?} → {t_large:?})"
    );
}

/// IDS by hand with non-maximal front coding: `shared` may be smaller than
/// the common prefix with the previous id (02 §2.8 allows it; the writer
/// never does it). `lookup` must still find every id. Regression: the fuzz
/// target found a mutated workload catalog whose IDS stayed valid and
/// ascending — one byte of an id changed, so the next id's `shared` was no
/// longer maximal — and `lookup` returned `None` for an id it holds.
#[test]
fn lookup_does_not_assume_maximal_front_coding() {
    let ids: [&[u8]; 7] = [b"a", b"ab", b"abc", b"abd", b"abda", b"b", b"ba"];
    let maximal: Vec<usize> = (0..ids.len())
        .map(|i| match i.checked_sub(1) {
            None => 0,
            Some(p) => ids[p]
                .iter()
                .zip(ids[i])
                .take_while(|(a, b)| a == b)
                .count(),
        })
        .collect();
    // Every id but the restart: `shared` maximal, zero, or one less.
    for choice in 0..3usize.pow(6) {
        let mut entries = Vec::new();
        let mut c = choice;
        for (i, id) in ids.iter().enumerate() {
            let shared = if i == 0 {
                0
            } else {
                let s = match c % 3 {
                    0 => maximal[i],
                    1 => 0,
                    _ => maximal[i].saturating_sub(1),
                };
                c /= 3;
                s
            };
            varint(u32_of(shared), &mut entries);
            varint(u32_of(id.len() - shared), &mut entries);
            entries.extend_from_slice(&id[shared..]);
        }
        let mut idx = 0u32.to_le_bytes().to_vec(); // one restart, at 0
        idx.extend_from_slice(&entries);
        let index = planes(&vec![(kind::SIMPLE << kind::SHIFT) | 3; ids.len()]);
        let mut sections = required(index, Vec::new(), Vec::new(), b"en\0x\0".to_vec());
        sections.insert(5, (section::IDS, idx));
        let cat = Catalog::new(raw(ids.len(), &sections), RAW_HASH).expect("loads");
        for (i, id) in ids.iter().enumerate() {
            let id = std::str::from_utf8(id).expect("ASCII");
            let want = MsgId::new(0, u32_of(i));
            assert_eq!(cat.lookup(id), want, "{id:?}, choice {choice}");
        }
        for miss in ["", "aa", "abe", "abda0", "bb", "c"] {
            assert_eq!(cat.lookup(miss), None, "{miss:?}, choice {choice}");
        }
    }
}
