//! P0.5 suite harness: runs WG test files through a minimal MF2 evaluator
//! whose numeric functions come from `numcore` (neutral output). Dev-only.
//!
//! Usage: suite <file.json>...   (paths relative to the repo root are fine)

mod parse;

use std::collections::HashMap;

use numcore as nc;
use parse::{Body, Decl, Expr, Key, Operand, Part};
use serde_json::Value as J;

#[derive(Clone)]
pub enum Val {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Num(nc::NumValue),
    Loc(numloc::LocValue),
    /// Fallback value; the string is the fallback representation without braces.
    Fallback(String),
}

#[derive(Default)]
pub struct Errs(pub Vec<&'static str>);
impl nc::ErrSink for Errs {
    fn push(&mut self, e: nc::Error) {
        self.0.push(match e {
            nc::Error::BadOperand => "bad-operand",
            nc::Error::BadOption => "bad-option",
            nc::Error::BadSelector => "bad-selector",
            nc::Error::UnsupportedOperation => "unsupported-operation",
            nc::Error::BadVariantKey => "bad-variant-key",
        });
    }
}

/// Owned option value handed to a backend.
pub enum OptV {
    Lit(String),
    Str(String),
    Int(i64),
}

impl OptV {
    pub fn as_nc(&self) -> nc::OptValue<'_> {
        match self {
            OptV::Lit(s) => nc::OptValue::Literal(s),
            OptV::Str(s) => nc::OptValue::VarStr(s),
            OptV::Int(n) => nc::OptValue::VarInt(*n),
        }
    }
}

/// en-US plural rules — harness stub only (the evaluator is P0.4's job).
pub fn en_category(op: &nc::PluralOperands, ordinal: bool) -> &'static str {
    if ordinal {
        let n10 = op.i % 10;
        let n100 = op.i % 100;
        if op.v != 0 {
            return "other";
        }
        return match (n10, n100) {
            (1, x) if x != 11 => "one",
            (2, x) if x != 12 => "two",
            (3, x) if x != 13 => "few",
            _ => "other",
        };
    }
    if op.i == 1 && op.v == 0 { "one" } else { "other" }
}

/// The function set under test.
pub trait Backend {
    /// `None` = unknown function.
    fn call(&self, name: &str, operand: Option<&Val>, opts: &[(String, OptV)], e: &mut Errs) -> Option<Option<Val>>;
    fn format(&self, v: &Val, e: &mut Errs) -> String;
    /// Spec Match; `Err` = Bad Variant Key.
    fn matches(&self, v: &Val, key: &str, e: &mut Errs) -> Result<bool, ()>;
    fn selectable(&self, v: &Val) -> bool;
}

pub fn operand_of(v: Option<&Val>) -> nc::Operand<'_> {
    match v {
        Some(Val::Str(s)) => nc::Operand::Str(s),
        Some(Val::Int(n)) => nc::Operand::Int(*n),
        Some(Val::Float(f)) => nc::Operand::Float(*f),
        Some(Val::Num(nv)) => nc::Operand::Num(nv),
        Some(Val::Loc(lv)) => nc::Operand::Num(&lv.num),
        _ => nc::Operand::Other,
    }
}

/// `fn-number` on: localized output from catalog-borne data for one locale.
pub struct Loc {
    pub data: Vec<u8>,
}

struct Str(String);
impl numloc::StrSink for Str {
    fn push_str(&mut self, s: &str) {
        self.0.push_str(s);
    }
    fn push_char(&mut self, c: char) {
        self.0.push(c);
    }
}

impl Backend for Loc {
    fn call(&self, name: &str, operand: Option<&Val>, opts: &[(String, OptV)], e: &mut Errs) -> Option<Option<Val>> {
        let o: Vec<(&str, nc::OptValue<'_>)> = opts.iter().map(|(k, v)| (k.as_str(), v.as_nc())).collect();
        let lop = match operand {
            Some(Val::Loc(lv)) => numloc::LocOperand::Loc(lv),
            other => numloc::LocOperand::Core(operand_of(other)),
        };
        let core_func = match name {
            "number" => Some(nc::Func::Number),
            "integer" => Some(nc::Func::Integer),
            "offset" => Some(nc::Func::Offset),
            _ => None,
        };
        if let Some(f) = core_func {
            let op = match operand {
                Some(Val::Loc(lv)) => nc::Operand::Num(&lv.num),
                other => operand_of(other),
            };
            return Some(nc::resolve(f, &op, &o, e).map(|num| Val::Loc(numloc::LocValue { num, kind: numloc::Kind::Number })));
        }
        Some(match name {
            "percent" => numloc::resolve_percent(&lop, &o, e).map(Val::Loc),
            "currency" => numloc::currency::resolve(&lop, &o, &self.data, e).map(Val::Loc),
            "unit" => numloc::unit::resolve(&lop, &o, e).map(Val::Loc),
            _ => return None,
        })
    }
    fn format(&self, v: &Val, e: &mut Errs) -> String {
        match v {
            Val::Loc(lv) => {
                let mut s = Str(String::new());
                numloc::format(lv, &self.data, false, &en_category, &mut s, e);
                s.0
            }
            other => Core.format(other, e),
        }
    }
    fn matches(&self, v: &Val, key: &str, e: &mut Errs) -> Result<bool, ()> {
        match v {
            Val::Loc(lv) => Core.matches(&Val::Num(lv.num.clone()), key, e),
            other => Core.matches(other, key, e),
        }
    }
    fn selectable(&self, v: &Val) -> bool {
        match v {
            Val::Loc(lv) => lv.num.selectable,
            other => Core.selectable(other),
        }
    }
}

pub struct ByteString(pub String);
impl nc::ByteSink for ByteString {
    fn byte(&mut self, b: u8) {
        self.0.push(char::from(b));
    }
}

/// Core: `:number`, `:integer`, `:offset` with neutral output.
pub struct Core;
impl Backend for Core {
    fn call(&self, name: &str, operand: Option<&Val>, opts: &[(String, OptV)], e: &mut Errs) -> Option<Option<Val>> {
        let func = match name {
            "number" => nc::Func::Number,
            "integer" => nc::Func::Integer,
            "offset" => nc::Func::Offset,
            _ => return None,
        };
        let o: Vec<(&str, nc::OptValue<'_>)> = opts.iter().map(|(k, v)| (k.as_str(), v.as_nc())).collect();
        Some(nc::resolve(func, &operand_of(operand), &o, e).map(Val::Num))
    }
    fn format(&self, v: &Val, e: &mut Errs) -> String {
        match v {
            Val::Num(nv) => {
                let f = nc::format_digits(nv, e);
                let mut s = ByteString(String::new());
                nc::write_neutral(&f, &mut s);
                s.0
            }
            other => plain(other),
        }
    }
    fn matches(&self, v: &Val, key: &str, e: &mut Errs) -> Result<bool, ()> {
        let Val::Num(nv) = v else { return Ok(false) };
        let f = nc::format_digits(nv, &mut Errs::default());
        nc::matches(nv, &f, key, &en_category).map_err(|err| nc::ErrSink::push(e, err))
    }
    fn selectable(&self, v: &Val) -> bool {
        matches!(v, Val::Num(nv) if nv.selectable)
    }
}

pub fn plain(v: &Val) -> String {
    match v {
        Val::Str(s) => s.clone(),
        Val::Int(n) => n.to_string(),
        Val::Float(f) => f.to_string(),
        Val::Bool(b) => b.to_string(),
        Val::Num(_) | Val::Loc(_) => "<num>".into(),
        Val::Fallback(r) => format!("{{{r}}}"),
    }
}

struct Ctx<'a> {
    params: HashMap<String, J>,
    locals: HashMap<String, Val>,
    e: Errs,
    b: &'a dyn Backend,
}

fn escape_lit(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

impl Ctx<'_> {
    fn lookup(&mut self, n: &str) -> Val {
        if let Some(v) = self.locals.get(n) {
            return v.clone();
        }
        match self.params.get(n) {
            Some(J::Number(x)) => x.as_i64().map_or_else(|| Val::Float(x.as_f64().unwrap_or(0.0)), Val::Int),
            Some(J::String(s)) => Val::Str(s.clone()),
            Some(J::Bool(b)) => Val::Bool(*b),
            Some(other) => Val::Str(other.to_string()),
            None => {
                self.e.0.push("unresolved-variable");
                Val::Fallback(format!("${n}"))
            }
        }
    }

    fn eval(&mut self, x: &Expr) -> Val {
        let (operand, repr) = match &x.operand {
            None => (None, format!(":{}", x.ann.as_ref().map_or("", |a| a.name.as_str()))),
            Some(Operand::Lit(s)) => (Some(Val::Str(s.clone())), format!("|{}|", escape_lit(s))),
            Some(Operand::Var(n)) => (Some(self.lookup(n)), format!("${n}")),
        };
        if let Some(Val::Fallback(_)) = operand {
            return Val::Fallback(repr);
        }
        let Some(ann) = &x.ann else {
            return operand.unwrap_or(Val::Fallback(repr));
        };
        let mut opts = Vec::new();
        for (k, v) in &ann.opts {
            let ov = match v {
                Operand::Lit(s) => OptV::Lit(s.clone()),
                Operand::Var(n) => match self.lookup(n) {
                    Val::Str(s) => OptV::Str(s),
                    Val::Int(i) => OptV::Int(i),
                    Val::Float(f) => OptV::Str(f.to_string()),
                    Val::Bool(b) => OptV::Str(b.to_string()),
                    v @ (Val::Num(_) | Val::Loc(_)) => OptV::Str(self.b.format(&v, &mut Errs::default())),
                    Val::Fallback(_) => continue,
                },
            };
            opts.push((k.clone(), ov));
        }
        match self.b.call(&ann.name, operand.as_ref(), &opts, &mut self.e) {
            None => {
                self.e.0.push("unknown-function");
                Val::Fallback(repr)
            }
            Some(None) => Val::Fallback(repr),
            Some(Some(v)) => v,
        }
    }

    fn pattern(&mut self, parts: &[Part]) -> String {
        let mut out = String::new();
        for p in parts {
            match p {
                Part::Text(t) => out.push_str(t),
                Part::Expr(x) => {
                    let v = self.eval(x);
                    out.push_str(&match v {
                        Val::Fallback(r) => format!("{{{r}}}"),
                        v => self.b.format(&v, &mut self.e),
                    });
                }
            }
        }
        out
    }
}

/// Run one message; returns (output, errors).
pub fn run(src: &str, params: &[J], b: &dyn Backend) -> (String, Vec<&'static str>) {
    let msg = match parse::parse(src) {
        Ok(m) => m,
        Err(err) => return (format!("<parse error: {err}>"), vec!["syntax-error"]),
    };
    let mut cx = Ctx { params: HashMap::new(), locals: HashMap::new(), e: Errs::default(), b };
    for p in params {
        if let (Some(n), Some(v)) = (p["name"].as_str(), p.get("value")) {
            cx.params.insert(n.to_owned(), v.clone());
        }
    }
    for d in &msg.decls {
        match d {
            Decl::Input(n, x) | Decl::Local(n, x) => {
                let v = cx.eval(x);
                cx.locals.insert(n.clone(), v);
            }
        }
    }
    let out = match &msg.body {
        Body::Pattern(p) => cx.pattern(p),
        Body::Match(sels, variants) => {
            let mut rvs: Vec<Option<Val>> = Vec::new();
            for s in sels {
                let v = cx.lookup(s);
                if b.selectable(&v) {
                    rvs.push(Some(v));
                } else {
                    cx.e.0.push("bad-selector");
                    rvs.push(None);
                }
            }
            // Rank each variant: per selector 0 = exact literal, 1 = keyword, 2 = `*`.
            let mut best: Option<(Vec<u8>, usize)> = None;
            for (idx, var) in variants.iter().enumerate() {
                let mut rank = Vec::new();
                let mut ok = true;
                for (k, rv) in var.keys.iter().zip(&rvs) {
                    match (k, rv) {
                        (Key::Star, _) => rank.push(2),
                        (Key::Lit(_), None) => ok = false,
                        (Key::Lit(key), Some(v)) => match b.matches(v, key, &mut cx.e) {
                            Ok(true) => rank.push(if nc::is_number_literal(key) { 0 } else { 1 }),
                            _ => ok = false,
                        },
                    }
                }
                if ok && best.as_ref().is_none_or(|(r, _)| rank < *r) {
                    best = Some((rank, idx));
                }
            }
            match best {
                Some((_, idx)) => cx.pattern(&variants[idx].pattern),
                None => String::new(),
            }
        }
    };
    (out, cx.e.0)
}

pub struct Outcome {
    pub pass: usize,
    pub fail: usize,
    pub lines: Vec<String>,
}

/// Run a suite file; `exp` is compared only when present, errors always.
pub fn run_file(path: &str, b: &dyn Backend) -> Outcome {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let file: J = serde_json::from_str(&text).expect("json");
    let defaults = &file["defaultTestProperties"];
    let mut o = Outcome { pass: 0, fail: 0, lines: Vec::new() };
    for t in file["tests"].as_array().expect("tests") {
        let src = t["src"].as_str().unwrap_or("");
        let params = t["params"].as_array().cloned().unwrap_or_default();
        let (out, mut errs) = run(src, &params, b);
        let exp = t.get("exp").or(defaults.get("exp")).and_then(J::as_str);
        let mut exp_errs: Vec<&str> = t
            .get("expErrors")
            .or(defaults.get("expErrors"))
            .and_then(J::as_array)
            .map(|a| a.iter().filter_map(|e| e["type"].as_str()).collect())
            .unwrap_or_default();
        errs.sort_unstable();
        exp_errs.sort_unstable();
        let out_ok = exp.is_none_or(|x| x == out);
        let ok = out_ok && errs == exp_errs;
        if ok { o.pass += 1 } else { o.fail += 1 }
        o.lines.push(format!(
            "{} {src:?} → {out:?} errs={errs:?}{}{}",
            if ok { "PASS" } else { "FAIL" },
            exp.map(|x| format!(" exp={x:?}")).unwrap_or_else(|| " (no exp)".into()),
            if errs == exp_errs { String::new() } else { format!(" expErrors={exp_errs:?}") },
        ));
    }
    o
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--loc <file>`: fn-number on, with that locale's LOCALE entries.
    let backend: Box<dyn Backend> = match args.iter().position(|a| a == "--loc") {
        Some(i) => {
            let path = args.remove(i + 1);
            args.remove(i);
            Box::new(Loc { data: std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}")) })
        }
        None => Box::new(Core),
    };
    let files = args;
    let mut total = (0, 0);
    for f in &files {
        let o = run_file(f, backend.as_ref());
        for l in &o.lines {
            println!("{l}");
        }
        println!("== {f}: {} pass, {} fail\n", o.pass, o.fail);
        total.0 += o.pass;
        total.1 += o.fail;
    }
    println!("TOTAL {} pass, {} fail", total.0, total.1);
}
