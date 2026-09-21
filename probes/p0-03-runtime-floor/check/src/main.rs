//! `p03` — native evidence that the P0.3 runtime works:
//! 1. every workload message × 4 locales × several argument sets, formatted
//!    from the catalog, equals an independent reference formatter that walks
//!    the parsed data model (not the catalog);
//! 2. parts output concatenates to the string output;
//! 3. hand-written spot checks (select, plural per locale, bidi, markup,
//!    fallback) and a mini corpus for the paths the workload lacks;
//! 4. a mutation run: corrupted catalogs never panic.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use p03_rt::{Arg, BidiStrategy, Catalog, FormatError, Formatter, MsgId, OptValue, Part, PartSink, ValueKind};
use p07_enc::manifest::Manifest;
use p07_enc::model::{Decl, Expr, Key, Message, Operand, Part as MPart};
use p07_enc::write::Layout;
use p07_enc::{LOCALES, LocaleData, default_corpus_dir, load_corpus};
use plural_rules::rule::Rule;

// ---------------------------------------------------------------- reference

#[derive(Clone, Copy)]
enum RArg<'a> {
    Str(&'a str),
    Int(i64),
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Raw,
    Str,
    Plural,
    Ordinal,
    Exact,
}

#[derive(Clone)]
enum Rv {
    Str(String, bool),
    Num(i64, Mode),
    Fb(String),
}

struct Reference<'a> {
    args: &'a HashMap<String, RArg<'a>>,
    rtl: bool,
    cardinal: &'a [Rule],
    ordinal: &'a [Rule],
    env: HashMap<String, Rv>,
    errors: Vec<&'static str>,
}

fn lit_src(s: &str) -> String {
    format!("|{}|", s.replace('\\', "\\\\").replace('|', "\\|"))
}

impl Reference<'_> {
    fn lookup(&mut self, v: &str) -> Rv {
        if let Some(x) = self.env.get(v) {
            return x.clone();
        }
        match self.args.get(v) {
            Some(RArg::Str(s)) => Rv::Str((*s).to_owned(), false),
            Some(RArg::Int(n)) => Rv::Num(*n, Mode::Raw),
            None => {
                self.errors.push("unresolved");
                Rv::Fb(format!("${v}"))
            }
        }
    }

    fn expr(&mut self, e: &Expr, given: Option<(Rv, String)>) -> Rv {
        let (operand, src) = match &e.operand {
            Some(Operand::Literal(s)) => (Some(Rv::Str(s.clone(), false)), lit_src(s)),
            Some(Operand::Variable(v)) => (Some(self.lookup(v)), format!("${v}")),
            None => match given {
                Some((v, s)) => (Some(v), s),
                None => (None, String::new()),
            },
        };
        let Some(f) = &e.function else {
            return operand.unwrap_or(Rv::Fb("\u{FFFD}".into()));
        };
        let fsrc = if src.is_empty() { format!(":{}", f.name) } else { src.clone() };
        let mut mode = Mode::Plural;
        for (k, v) in &f.options {
            if k == "select"
                && let Operand::Literal(v) = v
            {
                mode = match v.as_str() {
                    "ordinal" => Mode::Ordinal,
                    "exact" => Mode::Exact,
                    _ => Mode::Plural,
                };
            }
        }
        match (f.name.as_str(), operand) {
            (_, Some(Rv::Fb(s))) => Rv::Fb(s),
            ("string", Some(Rv::Str(s, _))) => Rv::Str(s, true),
            ("string", Some(Rv::Num(n, _))) => Rv::Num(n, Mode::Str),
            ("integer" | "number", Some(Rv::Num(n, _))) => Rv::Num(n, mode),
            ("integer" | "number", Some(Rv::Str(s, _))) => {
                let int = if f.name == "integer" { s.split('.').next().unwrap_or("") } else { s.as_str() };
                match int.parse::<i64>() {
                    Ok(n) => Rv::Num(n, mode),
                    Err(_) => {
                        self.errors.push("bad-operand");
                        Rv::Fb(src)
                    }
                }
            }
            ("string" | "integer" | "number", None) => {
                self.errors.push("bad-operand");
                Rv::Fb(fsrc)
            }
            _ => {
                self.errors.push("unknown-function");
                Rv::Fb(fsrc)
            }
        }
    }

    fn category(&self, n: i64, ordinal: bool) -> &'static str {
        let o = plural_rules::reference::operands(&n.unsigned_abs().to_string()).expect("operands");
        plural_rules::reference::select(if ordinal { self.ordinal } else { self.cardinal }, &o).as_str()
    }

    fn matches(&self, v: &Rv, key: &str) -> bool {
        let exact = |n: i64| key.parse::<i64>().ok() == Some(n) && key.trim_start_matches('-').bytes().all(|b| b.is_ascii_digit());
        match v {
            Rv::Str(s, true) => s == key,
            Rv::Num(n, Mode::Str) => n.to_string() == key,
            Rv::Num(n, Mode::Exact) => exact(*n),
            Rv::Num(n, Mode::Plural) => exact(*n) || self.category(*n, false) == key,
            Rv::Num(n, Mode::Ordinal) => exact(*n) || self.category(*n, true) == key,
            _ => false,
        }
    }

    fn format(&mut self, msg: &Message) -> String {
        for d in msg.decls() {
            match d {
                Decl::Input { name, expr } => {
                    let given = self.lookup(name);
                    let v = self.expr(expr, Some((given, format!("${name}"))));
                    self.env.insert(name.clone(), v);
                }
                Decl::Local { name, expr } => {
                    let v = self.expr(expr, None);
                    self.env.insert(name.clone(), v);
                }
            }
        }
        let pattern = match msg {
            Message::Pattern { pattern, .. } => pattern.clone(),
            Message::Select { selectors, variants, .. } => {
                let res: Vec<Option<Rv>> = selectors
                    .iter()
                    .map(|s| {
                        let v = self.lookup(s);
                        let ok = matches!(v, Rv::Str(_, true)) || matches!(v, Rv::Num(_, m) if m != Mode::Raw);
                        if !ok {
                            self.errors.push("bad-selector");
                        }
                        ok.then_some(v)
                    })
                    .collect();
                let matching: Vec<_> = variants
                    .iter()
                    .filter(|v| {
                        v.keys.iter().zip(&res).all(|(k, r)| match k {
                            Key::CatchAll => true,
                            Key::Literal(k) => r.as_ref().is_some_and(|r| self.matches(r, k)),
                        })
                    })
                    .collect();
                // Spec "Compare Variants": keep the first best in source order.
                let better = |a: &[Key], b: &[Key]| -> bool {
                    for ((k1, k2), r) in a.iter().zip(b).zip(&res) {
                        match (k1, k2) {
                            (Key::CatchAll, Key::Literal(_)) => return false,
                            (Key::Literal(_), Key::CatchAll) => return true,
                            (Key::CatchAll, Key::CatchAll) => continue,
                            (Key::Literal(x), Key::Literal(y)) if x == y => continue,
                            (Key::Literal(x), Key::Literal(y)) => {
                                return match r {
                                    Some(Rv::Num(n, Mode::Plural | Mode::Ordinal)) => {
                                        x.parse::<i64>().ok() == Some(*n) && y.parse::<i64>().ok() != Some(*n)
                                    }
                                    _ => false,
                                };
                            }
                        }
                    }
                    false
                };
                let mut best = matching.first().copied().expect("a catch-all variant");
                for v in matching.iter().skip(1) {
                    if better(&v.keys, &best.keys) {
                        best = v;
                    }
                }
                best.pattern.clone()
            }
        };
        let mut out = String::new();
        for p in &pattern {
            match p {
                MPart::Text(t) => out.push_str(t),
                MPart::Markup(_) => {}
                MPart::Expr(e) => {
                    let v = self.expr(e, None);
                    let ltr = matches!(v, Rv::Num(_, m) if m != Mode::Raw && m != Mode::Str);
                    let (open, close) = if ltr && !self.rtl {
                        ("", "")
                    } else if ltr {
                        ("\u{2066}", "\u{2069}")
                    } else {
                        ("\u{2068}", "\u{2069}")
                    };
                    out.push_str(open);
                    match v {
                        Rv::Str(s, _) => out.push_str(&s),
                        Rv::Num(n, _) => out.push_str(&n.to_string()),
                        Rv::Fb(s) => {
                            out.push('{');
                            out.push_str(&s);
                            out.push('}');
                        }
                    }
                    out.push_str(close);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- runtime side

/// Collects parts and renders them back to a string (to cross-check `parts`).
#[derive(Default)]
struct Collect {
    text: String,
    log: Vec<String>,
}

impl PartSink for Collect {
    fn part(&mut self, p: Part<'_>) {
        match p {
            Part::Text(t) => {
                self.text.push_str(t);
                self.log.push(format!("text:{t}"));
            }
            Part::BidiIsolation(i) => {
                self.text.push_str(i.as_str());
                self.log.push(format!("bidi:{:04X}", i.as_str().chars().next().map_or(0, |c| c as u32)));
            }
            Part::Expression { kind, value, .. } => {
                self.text.push_str(value);
                let k = if kind == ValueKind::Number { "number" } else { "string" };
                self.log.push(format!("{k}:{value}"));
            }
            Part::Fallback(src) => {
                let mut s = String::new();
                src.write(&mut s);
                self.text.push('{');
                self.text.push_str(&s);
                self.text.push('}');
                self.log.push(format!("fallback:{s}"));
            }
            Part::Markup { kind, name, options } => {
                let k = match kind {
                    p03_rt::MarkupKind::Open => "open",
                    p03_rt::MarkupKind::Standalone => "standalone",
                    p03_rt::MarkupKind::Close => "close",
                };
                let opts: Vec<String> = options
                    .map(|(n, v)| match v {
                        OptValue::Literal(l) => format!("{n}={l}"),
                        OptValue::Arg(Arg::Str(s)) => format!("{n}=${s}"),
                        OptValue::Arg(Arg::Int(i)) => format!("{n}=${i}"),
                        OptValue::Arg(Arg::Unset) => format!("{n}=$?"),
                    })
                    .collect();
                self.log.push(format!("markup-{k}:{name}{}", if opts.is_empty() { String::new() } else { format!("[{}]", opts.join(",")) }));
            }
        }
    }
}

fn err_name(e: FormatError) -> &'static str {
    match e {
        FormatError::UnresolvedVariable => "unresolved",
        FormatError::UnknownFunction => "unknown-function",
        FormatError::BadOperand => "bad-operand",
        FormatError::BadOption => "bad-option",
        FormatError::BadSelector => "bad-selector",
        FormatError::UnsupportedOperation => "unsupported",
        FormatError::MissingMessage => "missing",
        FormatError::Corrupt => "corrupt",
    }
}

fn run(f: &Formatter<'_>, id: usize, args: &[Arg<'_>]) -> (String, Vec<&'static str>, Collect) {
    let mut out = String::new();
    let mut errs: Vec<FormatError> = Vec::new();
    f.write(MsgId(id as u32), args, &mut out, &mut errs);
    let mut parts = Collect::default();
    let mut perrs: Vec<FormatError> = Vec::new();
    f.parts(MsgId(id as u32), args, &mut parts, &mut perrs);
    (out, errs.into_iter().map(err_name).collect(), parts)
}

fn rules_for(cldr: &plural_rules::cldr::Cldr, tag: &str, kind: plural_rules::cldr::Kind) -> Vec<Rule> {
    cldr.resolve(kind, tag).map(|(_, r)| r.rules.clone()).unwrap_or_default()
}

fn show(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2066}' => "⟨LRI⟩".to_owned(),
            '\u{2067}' => "⟨RLI⟩".to_owned(),
            '\u{2068}' => "⟨FSI⟩".to_owned(),
            '\u{2069}' => "⟨PDI⟩".to_owned(),
            '\u{202E}' => "⟨RLO⟩".to_owned(),
            '\u{202C}' => "⟨PDF⟩".to_owned(),
            c => c.to_string(),
        })
        .collect()
}

struct Tally {
    ok: usize,
    bad: usize,
}

impl Tally {
    fn check(&mut self, what: &str, got: &str, want: &str) {
        if got == want {
            self.ok += 1;
        } else {
            self.bad += 1;
            if self.bad <= 20 {
                println!("MISMATCH {what}\n   got  {}\n   want {}", show(got), show(want));
            }
        }
    }
}

const COUNTS: [i64; 10] = [0, 1, 2, 3, 5, 11, 21, 22, 101, 1000];

fn main() {
    let corpus = load_corpus(&default_corpus_dir(), &LOCALES).expect("corpus");
    let m = &corpus.manifest;
    let cldr = plural_rules::cldr::load(&plural_rules::cldr::default_dir()).expect("cldr");
    let mut tally = Tally { ok: 0, bad: 0 };
    let mut formats = 0usize;

    // 1 + 2: all messages, all locales, against the reference.
    println!("== workload: runtime (from the catalog) vs reference (from the data model)");
    let mut cats = Vec::new();
    for l in &corpus.locales {
        let (bytes, _) = l.write(m, Layout::RECOMMENDED).expect("write");
        let cat = Catalog::new(bytes, m.hash).unwrap_or_else(|_| panic!("load {}", l.tag));
        assert_eq!(cat.locale(), l.tag);
        cats.push(cat);
    }
    for (l, cat) in corpus.locales.iter().zip(&cats) {
        let f = Formatter::new(cat, BidiStrategy::Default);
        let card = rules_for(&cldr, &l.tag, plural_rules::cldr::Kind::Cardinal);
        let ord = rules_for(&cldr, &l.tag, plural_rules::cldr::Kind::Ordinal);
        let before = tally.bad;
        for (i, msg) in l.messages.iter().enumerate() {
            let Some(msg) = msg else { continue };
            let slots = &m.slots[i];
            let sets: Vec<Option<i64>> = if slots.iter().any(|s| s == "count") { COUNTS.iter().map(|&c| Some(c)).collect() } else { vec![Some(7)] };
            for set in sets.iter().copied().chain([None]) {
                // `None` = every argument unset (fallback path).
                let rargs: HashMap<String, RArg> = match set {
                    Some(c) => slots.iter().map(|s| (s.clone(), if s == "count" { RArg::Int(c) } else { RArg::Str("Ada") })).collect(),
                    None => HashMap::new(),
                };
                let args: Vec<Arg> = slots
                    .iter()
                    .map(|s| match rargs.get(s) {
                        Some(RArg::Str(x)) => Arg::Str(x),
                        Some(RArg::Int(n)) => Arg::Int(*n),
                        None => Arg::Unset,
                    })
                    .collect();
                let mut r = Reference { args: &rargs, rtl: l.rtl, cardinal: &card, ordinal: &ord, env: HashMap::new(), errors: Vec::new() };
                let want = r.format(msg);
                let (got, errs, parts) = run(&f, i, &args);
                formats += 2;
                tally.check(&format!("{} {} {:?}", l.tag, m.ids[i], set), &got, &want);
                tally.check(&format!("{} {} parts {:?}", l.tag, m.ids[i], set), &parts.text, &got);
                // Error *sets*: the runtime resolves declarations lazily and may
                // report one error once per use.
                let mut want_errs = r.errors.clone();
                want_errs.sort_unstable();
                want_errs.dedup();
                let mut got_errs = errs.clone();
                got_errs.sort_unstable();
                got_errs.dedup();
                tally.check(&format!("{} {} errors {:?}", l.tag, m.ids[i], set), &got_errs.join(","), &want_errs.join(","));
            }
        }
        println!("  {:6} {} messages, mismatches {}", l.tag, l.messages.len(), tally.bad - before);
    }

    // 3a: spot checks on the workload (hand-written expectations).
    println!("== spot checks");
    let id = |s: &str| m.msg_id(s).expect(s);
    let fmt = |loc: usize, s: &str, args: &[Arg]| run(&Formatter::new(&cats[loc], BidiStrategy::Default), id(s), args);
    let spots: Vec<(usize, &str, Vec<Arg>, &str)> = vec![
        (0, "admin.badge", vec![], "Failed"),
        (0, "admin.badges.upload", vec![Arg::Str("12")], "\u{2068}12\u{2069} participants"),
        (0, "admin.badges.upload", vec![Arg::Unset], "\u{2068}{$members}\u{2069} participants"),
        (0, "common.dialog.add", vec![Arg::Int(1)], "Sign enable reconnect 1 use action now."),
        (0, "common.dialog.add", vec![Arg::Int(5)], "Please 5 of fix previous international."),
        (0, "auth.upload.title", vec![], "Mode this."),
        (0, "app.canary.zq7-canary-msg", vec![Arg::Str("x")], "ZQ7-CANARY-TEXT-EN \u{2068}x\u{2069}"),
    ];
    for (loc, s, args, want) in &spots {
        let (got, _, _) = fmt(*loc, s, args);
        tally.check(&format!("spot {} {s}", corpus.locales[*loc].tag), &got, want);
        println!("  {:6} {s:28} → {}", corpus.locales[*loc].tag, show(&got));
    }
    // Plural categories per locale on one select message (which variant is chosen).
    let sel = id("common.dialog.add");
    for (loc, l) in corpus.locales.iter().enumerate() {
        let variants: Vec<String> = [0i64, 1, 2, 3, 5, 22, 25, 101]
            .iter()
            .map(|&n| {
                let (got, _, _) = fmt(loc, "common.dialog.add", &[Arg::Int(n)]);
                let Message::Select { variants, .. } = l.messages[sel].as_ref().unwrap() else { unreachable!() };
                let k = variants
                    .iter()
                    .find(|v| {
                        let mut r = String::new();
                        for p in &v.pattern {
                            if let MPart::Text(t) = p {
                                r.push_str(t);
                            }
                        }
                        let text: String = got.chars().filter(|c| !('\u{2066}'..='\u{2069}').contains(c)).collect();
                        let nlen = n.to_string();
                        text.replacen(&nlen, "", 1) == r
                    })
                    .map_or("?".to_owned(), |v| match &v.keys[0] {
                        Key::CatchAll => "*".into(),
                        Key::Literal(k) => k.clone(),
                    });
                format!("{n}→{k}")
            })
            .collect();
        println!("  {:6} common.dialog.add variants: {}", l.tag, variants.join(" "));
    }
    let (got, _, _) = fmt(3, "common.dialog.add", &[Arg::Int(1)]);
    println!("  ar-XB  common.dialog.add(1) → {}", show(&got));
    tally.check("ar-XB :integer isolated LRI in an RTL message", &got.contains("\u{2066}1\u{2069}").to_string(), "true");
    let (_, _, parts) = fmt(0, "auth.upload.title", &[]);
    println!("  en     auth.upload.title parts: {}", parts.log.join(" | "));
    tally.check("markup parts", &parts.log.join("|"), "text:Mode |markup-open:strong|text:this|markup-close:strong|text:.");
    // Bogus ids.
    for bogus in [1600u32, 1 << 24, u32::MAX] {
        let mut out = String::new();
        let mut errs: Vec<FormatError> = Vec::new();
        Formatter::new(&cats[0], BidiStrategy::Default).write(MsgId(bogus), &[], &mut out, &mut errs);
        tally.check(&format!("bogus id {bogus}"), &format!("{out}|{}", errs.iter().map(|e| err_name(*e)).collect::<Vec<_>>().join(",")), "|missing");
    }

    // 3b: mini corpus for paths the workload does not use.
    println!("== mini corpus (functions, literals, locals, exact/ordinal/multi-selector selection, markup options, RTL)");
    let mini_src: Vec<(&str, &str)> = vec![
        ("a.unknown-fn", "{$x :nope}"),
        ("b.literal-fallback", "{|a\\|b| :nope}"),
        ("c.function-only", "{:nope}"),
        ("d.local", ".local $y = {$x :integer} {{Y={$y}}}"),
        ("e.exact", ".input {$n :number select=exact}\n.match $n\n1 {{exact one}}\none {{category one}}\n* {{other}}"),
        ("f.exact-beats-category", ".input {$n :integer}\n.match $n\none {{category}}\n1 {{exact}}\n* {{other}}"),
        ("g.markup-options", "{#img src=|x.png| alt=$x /}and {#b}bold{/b}"),
        ("h.string-select", ".input {$s :string}\n.match $s\nfoo {{FOO}}\n* {{OTHER}}"),
        ("i.bad-selector", ".match $s\nfoo {{FOO}}\n* {{OTHER}}"),
        ("j.two-selectors", ".input {$a :integer}\n.input {$b :integer}\n.match $a $b\none one {{both}}\n1 * {{a1}}\n* 1 {{b1}}\n* * {{none}}"),
        ("k.ordinal", ".input {$n :integer select=ordinal}\n.match $n\none {{st}}\ntwo {{nd}}\nfew {{rd}}\n* {{th}}"),
        ("l.bidi", "{$x} and {$n :integer} and {|lit|}"),
        ("m.bad-operand", "{$x :integer}"),
        ("n.local-chain", ".local $a = {$x :string}\n.local $b = {$a}\n{{[{$b}]}}"),
    ];
    let parsed: Vec<(String, Message)> = mini_src.iter().map(|(i, s)| ((*i).to_owned(), p07_enc::parse::parse(s).expect(i))).collect();
    let mini = Manifest::build(&parsed, &[&parsed]).expect("manifest");
    let entries = |tag: &str| {
        vec![
            (1u32, plural_rules::encode::encode(&rules_for(&cldr, tag, plural_rules::cldr::Kind::Cardinal))),
            (2u32, plural_rules::encode::encode(&rules_for(&cldr, tag, plural_rules::cldr::Kind::Ordinal))),
        ]
    };
    let mk = |tag: &str, rtl: bool| -> Catalog {
        let l = LocaleData {
            tag: tag.to_owned(),
            rtl,
            source_bytes: 0,
            sources: vec![],
            messages: parsed.iter().map(|(_, msg)| Some(msg.clone())).collect(),
            locale_entries: entries(tag),
            plural_locale: tag.to_owned(),
        };
        let (bytes, _) = l.write(&mini, Layout::RECOMMENDED).expect("mini write");
        l.verify(&mini, Layout::RECOMMENDED, &bytes).expect("mini lossless");
        Catalog::new(bytes, mini.hash).unwrap_or_else(|_| panic!("mini load"))
    };
    let en = mk("en", false);
    let ar = mk("ar", true);
    let mid = |s: &str| mini.msg_id(s).expect(s);
    // (catalog, id, args by slot, expected string, expected errors)
    let cases: Vec<(&Catalog, &str, Vec<Arg>, &str, &str)> = vec![
        (&en, "a.unknown-fn", vec![Arg::Str("v")], "\u{2068}{$x}\u{2069}", "unknown-function"),
        (&en, "b.literal-fallback", vec![], "\u{2068}{|a\\|b|}\u{2069}", "unknown-function"),
        (&en, "c.function-only", vec![], "\u{2068}{:nope}\u{2069}", "unknown-function"),
        (&en, "d.local", vec![Arg::Int(42)], "Y=42", ""),
        (&en, "d.local", vec![Arg::Str("42")], "Y=42", ""),
        (&en, "e.exact", vec![Arg::Int(1)], "exact one", ""),
        (&en, "e.exact", vec![Arg::Int(2)], "other", ""),
        (&en, "f.exact-beats-category", vec![Arg::Int(1)], "exact", ""),
        (&en, "f.exact-beats-category", vec![Arg::Int(3)], "other", ""),
        (&en, "g.markup-options", vec![Arg::Str("pic")], "and bold", ""),
        (&en, "h.string-select", vec![Arg::Str("foo")], "FOO", ""),
        (&en, "h.string-select", vec![Arg::Str("bar")], "OTHER", ""),
        (&en, "i.bad-selector", vec![Arg::Str("foo")], "OTHER", "bad-selector"),
        (&en, "j.two-selectors", vec![Arg::Int(1), Arg::Int(1)], "a1", ""),
        (&en, "j.two-selectors", vec![Arg::Int(5), Arg::Int(1)], "b1", ""),
        (&en, "j.two-selectors", vec![Arg::Int(5), Arg::Int(5)], "none", ""),
        (&en, "k.ordinal", vec![Arg::Int(1)], "st", ""),
        (&en, "k.ordinal", vec![Arg::Int(2)], "nd", ""),
        (&en, "k.ordinal", vec![Arg::Int(23)], "rd", ""),
        (&en, "k.ordinal", vec![Arg::Int(11)], "th", ""),
        (&en, "l.bidi", vec![Arg::Int(5), Arg::Str("x")], "\u{2068}x\u{2069} and 5 and \u{2068}lit\u{2069}", ""),
        (&ar, "l.bidi", vec![Arg::Int(5), Arg::Str("x")], "\u{2068}x\u{2069} and \u{2066}5\u{2069} and \u{2068}lit\u{2069}", ""),
        (&en, "m.bad-operand", vec![Arg::Str("abc")], "\u{2068}{$x}\u{2069}", "bad-operand"),
        (&en, "n.local-chain", vec![Arg::Str("z")], "[\u{2068}z\u{2069}]", ""),
        (&en, "n.local-chain", vec![Arg::Unset], "[\u{2068}{$x}\u{2069}]", "unresolved"),
    ];
    for (cat, s, args, want, want_err) in &cases {
        let (got, errs, parts) = run(&Formatter::new(cat, BidiStrategy::Default), mid(s), args);
        let mut e = errs.clone();
        e.sort_unstable();
        e.dedup();
        tally.check(&format!("mini {s}"), &got, want);
        tally.check(&format!("mini {s} errors"), &e.join(","), want_err);
        tally.check(&format!("mini {s} parts"), &parts.text, &got);
        println!("  {:3} {s:24} → {:40} errors [{}]", cat.locale(), show(&got), e.join(","));
    }
    let (_, _, parts) = run(&Formatter::new(&en, BidiStrategy::Default), mid("g.markup-options"), &[Arg::Str("pic")]);
    println!("  g.markup-options parts: {}", parts.log.join(" | "));
    tally.check("markup options parts", &parts.log.join("|"), "markup-standalone:img[src=x.png,alt=$pic]|text:and |markup-open:b|text:bold|markup-close:b");
    let (got, _, parts) = run(&Formatter::new(&en, BidiStrategy::None), mid("l.bidi"), &[Arg::Int(5), Arg::Str("x")]);
    tally.check("bidi none", &got, "x and 5 and lit");
    println!("  l.bidi with BidiStrategy::None → {} ; parts {}", show(&got), parts.log.join(" | "));
    let (_, _, parts) = run(&Formatter::new(&en, BidiStrategy::Default), mid("a.unknown-fn"), &[Arg::Str("v")]);
    tally.check("fallback part", &parts.log.join("|"), "bidi:2068|fallback:$x|bidi:2069");
    let bad_hash = {
        let l0 = &corpus.locales[0];
        let (bytes, _) = l0.write(m, Layout::RECOMMENDED).expect("write");
        Catalog::new(bytes, m.hash ^ 1).is_err()
    };
    tally.check("manifest mismatch rejected (F6)", &bad_hash.to_string(), "true");

    // 4: mutation run — corrupt catalogs never panic.
    println!("== mutation run");
    let (base, _) = corpus.locales[0].write(m, Layout::RECOMMENDED).expect("write");
    let pool_start = {
        let n = u16::from_le_bytes([base[28], base[29]]) as usize;
        let last = 30 + (n - 1) * 10;
        u32::from_le_bytes(base[last + 2..last + 6].try_into().unwrap()) as usize
    };
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut rejected, mut loaded, mut panics, mut mutated_formats) = (0, 0, 0, 0usize);
    const ITERS: usize = 3000;
    for it in 0..ITERS {
        let mut b = base.clone();
        let flips = 1 + (rnd() % 4) as usize;
        for _ in 0..flips {
            // 90 % inside the structure (after the header), 10 % anywhere.
            let at = if rnd() % 10 == 0 { (rnd() as usize) % b.len() } else { 90 + (rnd() as usize) % (pool_start - 90) };
            b[at] ^= 1 << (rnd() % 8);
        }
        let r = catch_unwind(AssertUnwindSafe(|| {
            let Ok(cat) = Catalog::new(b, m.hash) else { return (false, 0) };
            let f = Formatter::new(&cat, BidiStrategy::Default);
            let mut n = 0;
            for i in 0..cat.len() {
                let mut out = String::new();
                let args = [Arg::Int(it as i64), Arg::Str("s"), Arg::Unset, Arg::Int(-1)];
                f.write(MsgId(i), &args, &mut out, &mut p03_rt::NoErrors);
                let mut c = Collect::default();
                f.parts(MsgId(i), &args, &mut c, &mut p03_rt::NoErrors);
                n += 2;
            }
            (true, n)
        }));
        match r {
            Ok((true, n)) => {
                loaded += 1;
                mutated_formats += n;
            }
            Ok((false, _)) => rejected += 1,
            Err(_) => panics += 1,
        }
    }
    println!("  {ITERS} mutated catalogs: {rejected} rejected by Catalog::new, {loaded} loaded and fully formatted ({mutated_formats} formats), {panics} panics");
    tally.check("mutation panics", &panics.to_string(), "0");

    println!("== total: {} checks passed, {} failed; {formats} workload formats", tally.ok, tally.bad);
    if tally.bad > 0 {
        std::process::exit(1);
    }
}
