//! The message plan: files, sections, ids, variables and source-locale text.
//!
//! Everything here is decided once per `(knobs, seed)`; locales, the app and
//! the statistics are all views of one [`Workload`].

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::error::Error;
use crate::knobs::Knobs;
use crate::rng::Rng;
use crate::shape::{self, apportion, quantile_pool, share};
use crate::text::{Lexicon, capitalize};
use crate::vocab;

/// The build canary (plans/06 §3, B6): an id, a variable name and a text that
/// must never appear in a client wasm. CI greps the final wasm for all three.
pub mod canary {
    /// Namespace (file) of the canary message.
    pub const FILE: &str = "app";
    /// Section head of the canary message.
    pub const SECTION: &str = "app.canary";
    /// Key of the canary message inside its section.
    pub const KEY: &str = "zq7-canary-msg";
    /// Full canary message id, exactly as a call site writes it.
    pub const MESSAGE_ID: &str = "app.canary.zq7-canary-msg";
    /// Canary variable name.
    pub const VARIABLE: &str = "zq7_canary_var";
    /// Prefix of every locale's canary text; the full text appends the locale
    /// tag, upper-cased (`ZQ7-CANARY-TEXT-EN`, `ZQ7-CANARY-TEXT-AR-XB`).
    pub const TEXT_PREFIX: &str = "ZQ7-CANARY-TEXT";

    /// The canary text of one locale.
    pub fn text(tag: &str) -> String {
        format!("{TEXT_PREFIX}-{}", tag.to_ascii_uppercase())
    }
}

/// What a variable holds; decides the values call sites pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    /// An integer.
    Num,
    /// A string.
    Str,
    /// A date/time (only with `--datetime`).
    Date,
}

impl VarKind {
    /// Lower-case name used in templates (`{{kind}}`) and `sites.json`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Num => "num",
            Self::Str => "str",
            Self::Date => "date",
        }
    }
}

/// The function annotation a variable's placeholder carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    /// `{$x}`.
    Plain,
    /// Declared `.input {$x :integer}` as a plural selector; placeholders are plain.
    Integer,
    /// `{$x :number …}`, option set 0–2.
    Number(u8),
    /// `{$x :datetime …}`, option set 0–2.
    DateTime(u8),
}

/// One external variable of a message.
#[derive(Debug, Clone)]
pub struct Var {
    /// MF2 name (also a valid Rust identifier).
    pub name: &'static str,
    /// Value kind.
    pub kind: VarKind,
    /// Annotation.
    pub func: Func,
}

/// One part of a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Literal text (never contains `{`, `}` or `\`).
    Text(String),
    /// A placeholder for `vars[i]`.
    Var(usize),
    /// `{#name}inner{/name}`.
    Markup {
        /// Markup name.
        name: &'static str,
        /// Text between open and close.
        inner: String,
    },
}

/// A pattern: a sequence of parts.
pub type Pattern = Vec<Part>;

/// A message body.
#[derive(Debug, Clone)]
pub enum Body {
    /// A pattern message.
    Pattern(Pattern),
    /// `.input {$v :integer} .match $v` with `(key, pattern)` variants; the
    /// last key is `*`.
    Select {
        /// Index of the selector variable.
        selector: usize,
        /// Variants in order.
        variants: Vec<(String, Pattern)>,
    },
}

/// One message.
#[derive(Debug, Clone)]
pub struct Message {
    /// Full dotted id, as a call site writes it.
    pub id: String,
    /// The part after the section prefix (what the `.mf2` entry line holds).
    pub key: String,
    /// File index.
    pub file: usize,
    /// Section index within the file.
    pub section: usize,
    /// External variables, in the order they first appear in the source.
    pub vars: Vec<Var>,
    /// Markup names used (empty for most messages).
    pub markup: Vec<&'static str>,
    /// This is the build canary.
    pub canary: bool,
    /// Source-locale body.
    pub source: Body,
}

impl Message {
    /// Whether the body is a `.match`.
    pub fn is_select(&self) -> bool {
        matches!(self.source, Body::Select { .. })
    }
}

/// A section of a file; `head == None` is the section-less block at the top
/// of the file.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section id (`chat.input`), if the block has a head.
    pub head: Option<String>,
    /// Message indices, in file order.
    pub messages: Vec<usize>,
}

/// One `.mf2` source file per locale.
#[derive(Debug, Clone)]
pub struct File {
    /// File stem (`chat` → `chat.mf2`).
    pub namespace: &'static str,
    /// Sections in order.
    pub sections: Vec<Section>,
}

/// The whole message plan.
#[derive(Debug, Clone)]
pub struct Workload {
    /// The knobs this plan was generated from.
    pub knobs: Knobs,
    /// Every message (index = generation order = file order).
    pub messages: Vec<Message>,
    /// Files in order.
    pub files: Vec<File>,
    /// Message indices sorted bytewise by id.
    pub order: Vec<usize>,
    /// `MsgId` of each message: its rank in `order` (the manifest's dense
    /// numbering, plans/05 §3).
    pub index: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    None,
    Number,
    DateTime,
}

#[derive(Debug, Clone, Copy)]
struct Class {
    vars: usize,
    select: bool,
    markup: bool,
    format: Format,
}

/// Rendered placeholder for a variable (`{$count}`, `{$amount :number}`).
pub fn placeholder(var: &Var) -> String {
    let name = var.name;
    match var.func {
        Func::Plain | Func::Integer => format!("{{${name}}}"),
        Func::Number(0) => format!("{{${name} :number}}"),
        Func::Number(1) => format!("{{${name} :number minimumFractionDigits=2}}"),
        Func::Number(_) => format!("{{${name} :number maximumFractionDigits=0}}"),
        Func::DateTime(0) => format!("{{${name} :datetime}}"),
        Func::DateTime(1) => format!("{{${name} :datetime dateLength=short}}"),
        Func::DateTime(_) => {
            format!("{{${name} :datetime dateFields=month-day timePrecision=hour}}")
        }
    }
}

fn markup_len(name: &str, inner: &str) -> usize {
    // {#name} inner {/name}
    2 * name.len() + 5 + inner.len()
}

/// Bytes of a select message outside its two English variant patterns:
/// `.input {$count :integer}\n.match $count\none {{` … `}}\n* {{` … `}}`.
fn select_overhead(var: &str) -> usize {
    let decl = format!(".input {{${var} :integer}}\n.match ${var}\n");
    decl.len() + "one {{}}\n".len() + "* {{}}".len()
}

impl Workload {
    /// Generates the plan for `knobs`.
    pub fn generate(knobs: &Knobs) -> Result<Self, Error> {
        validate(knobs)?;
        let seed = knobs.seed;
        let n = knobs.messages;
        let regular = n - 1; // the canary is the N-th message

        let classes = plan_classes(knobs)?;
        let mut files = plan_files(knobs, regular);

        // Slots: regular message j lives at files/sections in order.
        let mut placement: Vec<(usize, usize)> = Vec::with_capacity(regular);
        for (fi, file) in files.iter().enumerate() {
            for (si, section) in file.sections.iter().enumerate() {
                for _ in &section.messages {
                    placement.push((fi, si));
                }
            }
        }
        // Fill section message lists with the global indices.
        let mut next = 0usize;
        for file in &mut files {
            for section in &mut file.sections {
                for slot in &mut section.messages {
                    *slot = next;
                    next += 1;
                }
            }
        }
        debug_assert_eq!(next, regular);

        let ids = plan_ids(seed, &files, &placement);
        let vars = plan_vars(seed, &classes);
        let markup = plan_markup(seed, &classes);
        let lengths = plan_lengths(seed, &classes, &vars, &markup);

        let lex = Lexicon::english();
        let mut messages = Vec::with_capacity(n);
        for j in 0..regular {
            let mut rng = Rng::stream(seed, "en", j as u64);
            let class = classes[j];
            let source = if class.select {
                select_body(&mut rng, &lex, lengths[j], &vars[j])
            } else {
                pattern_body(&mut rng, &lex, lengths[j], &vars[j], markup[j].as_ref())
            };
            let (fi, si) = placement[j];
            messages.push(Message {
                id: ids[j].0.clone(),
                key: ids[j].1.clone(),
                file: fi,
                section: si,
                vars: vars[j].clone(),
                markup: markup[j].iter().map(|m| m.0).collect(),
                canary: false,
                source,
            });
        }

        // The canary: its own section at the end of the `app` file.
        let app = files
            .iter()
            .position(|f| f.namespace == canary::FILE)
            .expect("`app` is always the first namespace");
        files[app].sections.push(Section {
            head: Some(canary::SECTION.to_owned()),
            messages: vec![regular],
        });
        let canary_var = Var {
            name: canary::VARIABLE,
            kind: VarKind::Str,
            func: Func::Plain,
        };
        messages.push(Message {
            id: canary::MESSAGE_ID.to_owned(),
            key: canary::KEY.to_owned(),
            file: app,
            section: files[app].sections.len() - 1,
            vars: vec![canary_var],
            markup: Vec::new(),
            canary: true,
            source: Body::Pattern(vec![
                Part::Text(format!("{} ", canary::text("en"))),
                Part::Var(0),
            ]),
        });

        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| messages[a].id.as_bytes().cmp(messages[b].id.as_bytes()));
        let mut index = vec![0u32; n];
        for (rank, &m) in order.iter().enumerate() {
            index[m] = u32::try_from(rank).expect("fewer than 2^32 messages");
        }

        Ok(Self {
            knobs: knobs.clone(),
            messages,
            files,
            order,
            index,
        })
    }
}

fn validate(knobs: &Knobs) -> Result<(), Error> {
    if knobs.messages < 50 {
        return Err(Error::Knobs("--messages must be at least 50".into()));
    }
    if knobs.files == 0 || knobs.files > vocab::NAMESPACES.len() {
        return Err(Error::Knobs(format!(
            "--files must be between 1 and {}",
            vocab::NAMESPACES.len()
        )));
    }
    if knobs.locales == 0 || knobs.locales > vocab::REAL_LOCALES.len() + 1 {
        return Err(Error::Knobs(format!(
            "--locales must be between 1 and {}",
            vocab::REAL_LOCALES.len() + 1
        )));
    }
    Ok(())
}

fn plan_classes(knobs: &Knobs) -> Result<Vec<Class>, Error> {
    let n = knobs.messages;
    let mut counts = apportion(n, &shape::VAR_SHARES);
    // The canary is one of the 1-variable messages.
    counts[1] -= 1;
    let selects = share(n, shape::SELECT_SHARE).min(counts[1]);
    let markups = share(n, shape::MARKUP_SHARE).min(counts[0]);
    let formattable = counts[1] - selects + counts[2] + counts[3] + counts[4];
    if knobs.number + knobs.datetime > formattable {
        return Err(Error::Knobs(format!(
            "--number + --datetime must not exceed {formattable} (the non-select messages with variables)"
        )));
    }
    let mut classes = Vec::with_capacity(n - 1);
    for (vars, &count) in counts.iter().enumerate() {
        for i in 0..count {
            classes.push(Class {
                vars,
                select: vars == 1 && i < selects,
                markup: vars == 0 && i < markups,
                format: Format::None,
            });
        }
    }
    let mut rng = Rng::stream(knobs.seed, "classes", 0);
    rng.shuffle(&mut classes);
    let (mut number, mut datetime) = (knobs.number, knobs.datetime);
    for class in &mut classes {
        if class.vars == 0 || class.select {
            continue;
        }
        if number > 0 {
            class.format = Format::Number;
            number -= 1;
        } else if datetime > 0 {
            class.format = Format::DateTime;
            datetime -= 1;
        } else {
            break;
        }
    }
    Ok(classes)
}

fn plan_files(knobs: &Knobs, regular: usize) -> Vec<File> {
    let namespaces = &vocab::NAMESPACES[..knobs.files];
    let weights: Vec<u64> = namespaces.iter().map(|&(_, w)| u64::from(w)).collect();
    let per_file = apportion(regular, &weights);
    let mut files = Vec::with_capacity(namespaces.len());
    for (fi, (&(ns, _), &count)) in namespaces.iter().zip(&per_file).enumerate() {
        let mut rng = Rng::stream(knobs.seed, "sections", fi as u64);
        let mut words: Vec<&'static str> = vocab::SECTION_WORDS.to_vec();
        rng.shuffle(&mut words);
        let mut sections = Vec::new();
        let mut remaining = count;
        while remaining > 0 {
            let size = rng.range(4, 24).min(remaining);
            remaining -= size;
            let k = sections.len();
            let head = if k == 0 {
                if ns == vocab::TOPLEVEL_NAMESPACE {
                    None
                } else {
                    Some(ns.to_owned())
                }
            } else {
                let word = words[(k - 1) % words.len()];
                let round = (k - 1) / words.len();
                if round == 0 {
                    Some(format!("{ns}.{word}"))
                } else {
                    Some(format!("{ns}.{word}-{}", round + 1))
                }
            };
            sections.push(Section {
                head,
                messages: vec![0; size],
            });
        }
        files.push(File {
            namespace: ns,
            sections,
        });
    }
    files
}

/// Full id and key per regular message; mean full id length follows
/// [`shape::ID_KNOTS`] (a running correction absorbs sections whose prefix
/// is longer than the drawn length).
fn plan_ids(seed: u64, files: &[File], placement: &[(usize, usize)]) -> Vec<(String, String)> {
    let lex = Lexicon::keys();
    let mut pool = quantile_pool(placement.len(), shape::ID_KNOTS);
    Rng::stream(seed, "idlen", 0).shuffle(&mut pool);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(canary::MESSAGE_ID.to_owned());
    let mut carry: i64 = 0;
    let mut out = Vec::with_capacity(placement.len());
    for (j, &(fi, si)) in placement.iter().enumerate() {
        let head = files[fi].sections[si].head.as_deref();
        let prefix = head.map_or(0, |h| h.len() + 1);
        let want = i64::try_from(pool[j]).expect("small") + carry;
        let key_len = usize::try_from((want - i64::try_from(prefix).expect("small")).max(2))
            .expect("positive")
            .min(40);
        let mut rng = Rng::stream(seed, "key", j as u64);
        let mut chosen = None;
        for _ in 0..64 {
            let key = lex.key(&mut rng, key_len);
            let id = head.map_or_else(|| key.clone(), |h| format!("{h}.{key}"));
            if !seen.contains(&id) {
                chosen = Some((id, key));
                break;
            }
        }
        let (id, key) = chosen.unwrap_or_else(|| {
            // Tiny length buckets can run out of fresh words: number them.
            let base = lex.key(&mut rng, key_len);
            let mut k = 2usize;
            loop {
                let key = format!("{base}-{k}");
                let id = head.map_or_else(|| key.clone(), |h| format!("{h}.{key}"));
                if !seen.contains(&id) {
                    break (id, key);
                }
                k += 1;
            }
        });
        carry = want - i64::try_from(id.len()).expect("small");
        seen.insert(id.clone());
        out.push((id, key));
    }
    out
}

fn plan_vars(seed: u64, classes: &[Class]) -> Vec<Vec<Var>> {
    classes
        .iter()
        .enumerate()
        .map(|(j, class)| {
            let mut rng = Rng::stream(seed, "vars", j as u64);
            if class.select {
                return vec![Var {
                    name: "count",
                    kind: VarKind::Num,
                    func: Func::Integer,
                }];
            }
            let mut vars: Vec<Var> = Vec::with_capacity(class.vars);
            let option = u8::try_from(rng.below(3)).expect("< 3");
            for i in 0..class.vars {
                let var = loop {
                    let (kind, func, list) = match (i, class.format) {
                        (0, Format::Number) => {
                            (VarKind::Num, Func::Number(option), vocab::NUM_VARS)
                        }
                        (0, Format::DateTime) => {
                            (VarKind::Date, Func::DateTime(option), vocab::DATE_VARS)
                        }
                        _ if rng.chance(60) => (VarKind::Str, Func::Plain, vocab::STR_VARS),
                        _ => (VarKind::Num, Func::Plain, vocab::NUM_VARS),
                    };
                    let name = *rng.pick(list);
                    if vars.iter().all(|v| v.name != name) {
                        break Var { name, kind, func };
                    }
                };
                vars.push(var);
            }
            vars
        })
        .collect()
}

fn plan_markup(seed: u64, classes: &[Class]) -> Vec<Option<(&'static str, &'static str)>> {
    classes
        .iter()
        .enumerate()
        .map(|(j, class)| {
            class.markup.then(|| {
                let mut rng = Rng::stream(seed, "markup", j as u64);
                let &(name, inners) = rng.pick(vocab::MARKUP);
                (name, *rng.pick(inners))
            })
        })
        .collect()
}

/// Minimum source length a message needs for its placeholders and one word.
fn min_len(class: &Class, vars: &[Var], markup: Option<&(&str, &str)>) -> usize {
    if class.select {
        // Two variants, each `{$count}` + space + a word (+ period).
        return select_overhead(vars[0].name) + 2 * (placeholder(&vars[0]).len() + 4);
    }
    let mut tokens: usize = vars.iter().map(|v| placeholder(v).len() + 1).sum();
    if let Some((name, inner)) = markup {
        tokens += markup_len(name, inner) + 1;
    }
    tokens + 3
}

fn plan_lengths(
    seed: u64,
    classes: &[Class],
    vars: &[Vec<Var>],
    markup: &[Option<(&'static str, &'static str)>],
) -> Vec<usize> {
    let n = classes.len();
    let mut pool = quantile_pool(n, shape::LENGTH_KNOTS);
    if let Some(last) = pool.last_mut() {
        *last = shape::MAX_LENGTH;
    }
    let mut rng = Rng::stream(seed, "lengths", 0);
    let mut lengths = vec![0usize; n];

    // Selects take the longest lengths (they carry two variants).
    let selects: Vec<usize> = (0..n).filter(|&j| classes[j].select).collect();
    for &j in &selects {
        lengths[j] = pool.pop().expect("pool has room for selects");
    }

    // Then messages with placeholders or markup, most demanding first; each
    // draws uniformly among the remaining lengths that fit it.
    let mut demanding: Vec<(usize, usize)> = (0..n)
        .filter(|&j| !classes[j].select && (classes[j].vars > 0 || classes[j].markup))
        .map(|j| (min_len(&classes[j], &vars[j], markup[j].as_ref()), j))
        .collect();
    demanding.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (min, j) in demanding {
        let first = pool.partition_point(|&len| len < min);
        let len = if first < pool.len() {
            let k = rng.range(first, pool.len() - 1);
            pool.remove(k)
        } else {
            pool.pop().expect("pool not empty").max(min)
        };
        lengths[j] = len.max(min);
    }

    // The rest (simple messages) in random order.
    rng.shuffle(&mut pool);
    let mut rest = pool.into_iter();
    for len in &mut lengths {
        if *len == 0 {
            *len = rest.next().expect("one length per message");
        }
    }
    lengths
}

enum Token {
    Word(String),
    Part(Part),
}

/// A pattern of exactly `total` bytes: words around the given placeholder
/// parts, joined by single spaces; a sentence (capitalised, final period)
/// when `sentence`.
fn exact_pattern(
    rng: &mut Rng,
    lex: &Lexicon,
    total: usize,
    parts: Vec<(Part, usize)>,
    sentence: bool,
) -> Pattern {
    let part_bytes: usize = parts.iter().map(|p| p.1).sum();
    let mut word_bytes = total.saturating_sub(part_bytes + parts.len()).max(1);
    let period = sentence && word_bytes >= 4;
    if period {
        word_bytes -= 1;
    }
    let words = lex.fill(rng, word_bytes);
    let mut positions: Vec<usize> = parts.iter().map(|_| rng.below(words.len() + 1)).collect();
    positions.sort_unstable();

    let mut tokens: Vec<Token> = Vec::with_capacity(words.len() + parts.len());
    let mut parts = parts.into_iter().map(|p| p.0);
    let mut pi = 0;
    for (wi, word) in words.iter().enumerate() {
        while pi < positions.len() && positions[pi] == wi {
            tokens.push(Token::Part(parts.next().expect("one part per position")));
            pi += 1;
        }
        tokens.push(Token::Word((*word).to_owned()));
    }
    for part in parts {
        tokens.push(Token::Part(part));
    }
    if let Some(Token::Word(first)) = tokens.first_mut() {
        *first = capitalize(first);
    }

    let mut out: Pattern = Vec::new();
    let mut text = String::new();
    for (k, token) in tokens.into_iter().enumerate() {
        if k > 0 {
            text.push(' ');
        }
        match token {
            Token::Word(w) => text.push_str(&w),
            Token::Part(p) => {
                if !text.is_empty() {
                    out.push(Part::Text(std::mem::take(&mut text)));
                }
                out.push(p);
            }
        }
    }
    if period {
        text.push('.');
    }
    if !text.is_empty() {
        out.push(Part::Text(text));
    }
    out
}

fn pattern_body(
    rng: &mut Rng,
    lex: &Lexicon,
    total: usize,
    vars: &[Var],
    markup: Option<&(&'static str, &'static str)>,
) -> Body {
    let mut parts: Vec<(Part, usize)> = vars
        .iter()
        .enumerate()
        .map(|(i, v)| (Part::Var(i), placeholder(v).len()))
        .collect();
    if let Some(&(name, inner)) = markup {
        parts.push((
            Part::Markup {
                name,
                inner: inner.to_owned(),
            },
            markup_len(name, inner),
        ));
    }
    rng.shuffle(&mut parts);
    Body::Pattern(exact_pattern(rng, lex, total, parts, total >= 24))
}

fn select_body(rng: &mut Rng, lex: &Lexicon, total: usize, vars: &[Var]) -> Body {
    let budget = total - select_overhead(vars[0].name);
    let one = budget / 2;
    let other = budget - one;
    let ph = placeholder(&vars[0]).len();
    let variants = vec![
        (
            "one".to_owned(),
            exact_pattern(rng, lex, one, vec![(Part::Var(0), ph)], one >= 24),
        ),
        (
            "*".to_owned(),
            exact_pattern(rng, lex, other, vec![(Part::Var(0), ph)], other >= 24),
        ),
    ];
    Body::Select {
        selector: 0,
        variants,
    }
}

/// Escapes MF2 text (`\`, `{`, `}`).
pub fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        if matches!(c, '\\' | '{' | '}') {
            out.push('\\');
        }
        out.push(c);
    }
}

/// MF2 source of a pattern.
pub fn render_pattern(pattern: &[Part], vars: &[Var], out: &mut String) {
    for part in pattern {
        match part {
            Part::Text(t) => escape_text(t, out),
            Part::Var(i) => out.push_str(&placeholder(&vars[*i])),
            Part::Markup { name, inner } => {
                out.push_str("{#");
                out.push_str(name);
                out.push('}');
                escape_text(inner, out);
                out.push_str("{/");
                out.push_str(name);
                out.push('}');
            }
        }
    }
}

/// MF2 source of a body (select messages use LF line breaks).
pub fn render_body(body: &Body, vars: &[Var]) -> String {
    let mut out = String::new();
    match body {
        Body::Pattern(p) => render_pattern(p, vars, &mut out),
        Body::Select { selector, variants } => {
            let name = vars[*selector].name;
            let _ = write!(out, ".input {{${name} :integer}}\n.match ${name}\n");
            for (k, (key, pattern)) in variants.iter().enumerate() {
                if k > 0 {
                    out.push('\n');
                }
                out.push_str(key);
                out.push_str(" {{");
                render_pattern(pattern, vars, &mut out);
                out.push_str("}}");
            }
        }
    }
    out
}

/// The text a string-literal call site shows for a message: the pattern
/// source, or the catch-all variant of a select.
pub fn display_source(message: &Message) -> String {
    let mut out = String::new();
    match &message.source {
        Body::Pattern(p) => render_pattern(p, &message.vars, &mut out),
        Body::Select { variants, .. } => {
            if let Some((_, p)) = variants.last() {
                render_pattern(p, &message.vars, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Workload, render_body};
    use crate::knobs::Knobs;

    #[test]
    fn lengths_are_exact_and_ids_unique() {
        let wl = Workload::generate(&Knobs::default()).unwrap();
        assert_eq!(wl.messages.len(), 1600);
        let mut ids: Vec<&str> = wl.messages.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 1600);
        for m in &wl.messages {
            let src = render_body(&m.source, &m.vars);
            assert!(!src.starts_with(' ') && !src.ends_with(' '), "{src:?}");
            assert!(src.len() <= 259 || m.canary, "{} {}", m.id, src.len());
        }
    }
}
