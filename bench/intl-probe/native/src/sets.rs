//! The probe's data sets (see `main.rs` for the files).

use std::fmt::Write as _;
use std::path::Path;

use mf2::{Arg, FormatContext, FormatError, Formatter, Function, Registry, functions};
use mf2_locale_data::{PluralKind, plural_entry, plural_locales, plural_rules};
use mf2_runtime::{Operands, plural_category};
use serde_json::{Map, Value, json};

use crate::catalogs::{Blob, multi};
use crate::error::{Error, Result, io, json as json_err};

/// The locale panel (plans/11 A0, 06 §5 P0.5).
pub(crate) const PANEL: [&str; 11] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

/// Today's Rust registry: the core functions (neutral), or with feature
/// `fn-number` mf2-fn-number's localized :number / :integer / :offset and
/// :percent. Its :currency and :unit join in A4; until then they are
/// Unknown Function on the Rust side of the locale-symbol cases.
#[cfg(not(feature = "fn-number"))]
static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
#[cfg(feature = "fn-number")]
static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static CX: FormatContext = {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = mf2::BidiStrategy::None;
    cx
};

fn error_names(errs: &[FormatError]) -> String {
    let mut v: Vec<&str> = errs
        .iter()
        .map(|e| e.kind().map_or("other", mf2::ErrorKind::suite_name))
        .collect();
    v.sort_unstable();
    v.join(",")
}

// ───────────────────────────────────────────────────────────── speed ──

/// The timed messages: `(name, source)`; each has the one slot `n`.
pub(crate) const SPEED: [(&str, &str); 8] = [
    ("plain", "{$n}"),
    ("number", "{$n :number}"),
    (
        "number-frac2",
        "{$n :number minimumFractionDigits=2 maximumFractionDigits=2}",
    ),
    ("number-sig3", "{$n :number maximumSignificantDigits=3}"),
    ("integer", "{$n :integer}"),
    (
        "select",
        ".input {$n :integer} .match $n one {{one}} * {{other}}",
    ),
    (
        "select-3",
        ".input {$n :integer} .match $n one {{one}} few {{few}} many {{many}} * {{other}}",
    ),
    (
        "select-fmt",
        ".input {$n :integer} .match $n one {{{$n} item}} * {{{$n} items}}",
    ),
];

/// A timed catalog: plural entries, and with feature `number-data` the
/// number data `mf2-fn-number` formats with (the `rust-loc` variant).
fn speed_catalog(sources: &[String], locale: &str) -> Result<crate::catalogs::Written> {
    #[cfg(feature = "number-data")]
    {
        let mut entries = mf2_locale_data::plural_locale_entries(
            locale,
            &[PluralKind::Cardinal, PluralKind::Ordinal],
        )?;
        let mut needs = mf2_locale_data::NumberNeeds::default();
        needs.symbols = true;
        needs.percent = true;
        entries.extend(mf2_locale_data::number_locale_entries(locale, &needs)?);
        entries.sort_by_key(|(k, _)| *k);
        crate::catalogs::multi_with(sources, locale, entries)
    }
    #[cfg(not(feature = "number-data"))]
    multi(sources, locale)
}

pub(crate) fn speed(out: &Path) -> Result<()> {
    let sources: Vec<String> = SPEED.iter().map(|(_, s)| (*s).to_owned()).collect();
    let mut blob = Blob::default();
    for locale in ["en", "pl"] {
        blob.push(&speed_catalog(&sources, locale)?);
    }
    let messages: Vec<Value> = SPEED
        .iter()
        .map(|(name, src)| json!({"name": name, "src": src}))
        .collect();
    blob.write(
        out,
        "speed",
        json!({"messages": messages, "locales": ["en", "pl"]}),
    )
}

// ──────────────────────────────────────────────────────────────── L4 ──

/// The suite files the A0 L4 run covers.
pub(crate) const L4_FILES: [&str; 5] = ["number", "integer", "offset", "percent", "currency"];

pub(crate) fn l4(repo: &Path, out: &Path) -> Result<()> {
    let mut blob = Blob::default();
    let mut tests = Vec::new();
    for file in L4_FILES {
        let path = repo.join(format!(
            "third_party/message-format-wg/test/tests/functions/{file}.json"
        ));
        let text = std::fs::read_to_string(&path).map_err(io(&path))?;
        let doc: Value = serde_json::from_str(&text).map_err(json_err(&path))?;
        let defaults = doc
            .get("defaultTestProperties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let list = doc
            .get("tests")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::Input(format!("{}: no tests", path.display())))?;
        for (index, t) in list.iter().enumerate() {
            let mut p: Map<String, Value> = defaults.clone();
            if let Some(own) = t.as_object() {
                for (k, v) in own {
                    p.insert(k.clone(), v.clone());
                }
            }
            let src = p
                .get("src")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let locale = p
                .get("locale")
                .and_then(Value::as_str)
                .unwrap_or("en-US")
                .to_owned();
            let exp_errors: Vec<String> = p
                .get("expErrors")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.get("type").and_then(Value::as_str).map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let mut test = json!({
                "file": file,
                "index": index,
                "src": src,
                "locale": locale,
                "bidi": p.get("bidiIsolation").and_then(Value::as_str).unwrap_or("default"),
                "exp": p.get("exp").cloned().unwrap_or(Value::Null),
                "expErrors": exp_errors,
                "expParts": p.get("expParts").cloned().unwrap_or(Value::Null),
            });
            match mf2::compile_str(&src, &locale) {
                Ok(c) => {
                    let slots = c.manifest.slots.first().cloned().unwrap_or_default();
                    let params: Vec<Value> = slots
                        .iter()
                        .map(|slot| param(p.get("params"), slot))
                        .collect();
                    let cat = blob.push_compiled(&c);
                    test["cat"] = json!(cat);
                    test["params"] = Value::Array(params);
                }
                Err(e) => {
                    let kinds: Vec<&str> = e.kinds().iter().map(|k| k.suite_name()).collect();
                    test["rejected"] = json!(kinds);
                }
            }
            tests.push(test);
        }
    }
    blob.write(out, "l4", json!({"tests": tests}))
}

/// The argument of `slot` from a suite test's `params`, as the harness
/// takes it (conformance/src/l4.rs's rules: an untyped string is a string,
/// an integral number an `i64`, another number an `f64`, anything else an
/// application value with no conversions).
fn param(params: Option<&Value>, slot: &str) -> Value {
    let Some(Value::Array(ps)) = params else {
        return json!({"kind": "unset"});
    };
    for p in ps {
        if p.get("name").and_then(Value::as_str) != Some(slot) {
            continue;
        }
        let typed = p.get("type").and_then(Value::as_str);
        return match (typed, p.get("value")) {
            (None, Some(Value::String(s))) => json!({"kind": "str", "value": s}),
            (None, Some(Value::Number(n))) => match n.as_i64() {
                Some(i) => json!({"kind": "int", "value": i}),
                None => json!({"kind": "float", "value": n.as_f64()}),
            },
            _ => json!({"kind": "other"}),
        };
    }
    json!({"kind": "unset"})
}

// ──────────────────────────────────────────────────────────── plural ──

/// Every CLDR sample: `[kind, locale, sample, CLDR category, ours]` — ours
/// is `mf2_runtime::plural_category` over the shipped entry (the Phase 3
/// test asserts they agree on all 15,041; this re-checks it).
pub(crate) fn plural(out: &Path) -> Result<()> {
    let mut rows = Vec::new();
    let mut disagree = 0usize;
    for kind in PluralKind::ALL {
        for locale in plural_locales(kind)? {
            let lr = plural_rules(kind, locale)?;
            let entry = plural_entry(kind, locale)?;
            for rule in lr.rules {
                for list in [&rule.integer, &rule.decimal].into_iter().flatten() {
                    let samples = list
                        .expand()
                        .map_err(|e| Error::Input(format!("{locale}: {e}")))?;
                    for sample in samples {
                        let ours = Operands::parse(&sample)
                            .map_or("?", |o| plural_category(&entry, &o).as_str());
                        if ours != rule.category.as_str() {
                            disagree += 1;
                        }
                        rows.push(json!([
                            kind.name(),
                            locale,
                            sample,
                            rule.category.as_str(),
                            ours
                        ]));
                    }
                }
            }
        }
    }
    std::fs::create_dir_all(out).map_err(io(out))?;
    let path = out.join("plural.json");
    let doc = json!({"cldr": "48.2.1", "samples": rows});
    std::fs::write(&path, doc.to_string()).map_err(io(&path))?;
    eprintln!(
        "{}: {} samples, ours ≠ CLDR on {disagree}",
        path.display(),
        rows.len()
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────── ecma ──

/// A literal's source form (`|…|`, escaping `\` and `|`).
fn quoted(v: &str) -> String {
    let mut s = String::from("|");
    for c in v.chars() {
        if c == '\\' || c == '|' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('|');
    s
}

/// The display message of a `runtime-bench numbers ecma` line
/// (`{"v":…, "o":{…}, "out":…, "errs":…}`), as its `display_source`
/// writes it (option order does not matter: no case repeats an option).
fn ecma_source(line: &Value) -> Option<String> {
    let v = line.get("v")?.as_str()?;
    let mut s = format!("{{{} :number", quoted(v));
    for (k, val) in line.get("o")?.as_object()? {
        let text = match val {
            Value::String(t) => t.clone(),
            other => other.to_string(),
        };
        let _ = write!(s, " {k}=|{text}|");
    }
    s.push('}');
    Some(s)
}

pub(crate) fn ecma(input: &Path, out: &Path) -> Result<()> {
    let text = std::fs::read_to_string(input).map_err(io(input))?;
    let mut sources = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let line: Value = serde_json::from_str(l).map_err(json_err(input))?;
        sources.push(
            ecma_source(&line).ok_or_else(|| {
                Error::Input(format!("{}:{}: not a case", input.display(), i + 1))
            })?,
        );
    }
    let mut blob = Blob::default();
    for chunk in sources.chunks(10_000) {
        blob.push(&multi(chunk, "en")?);
    }
    blob.write(
        out,
        "ecma",
        json!({"cases": sources.len(), "chunk": 10_000}),
    )
}

// ─────────────────────────────────────────────────── locale symbols ──

/// Values of the locale-symbol cases.
pub(crate) const LOC_VALUES: [&str; 14] = [
    "0",
    "1",
    "-1",
    "1.5",
    "2",
    "3",
    "5",
    "21",
    "0.5",
    "1234",
    "1234.5",
    "12345.678",
    "-1234.56",
    "1000000",
];

/// Annotations of the locale-symbol cases (`{|v| …}`), then the two
/// selection messages (the category is the output).
pub(crate) const LOC_FUNCS: [&str; 24] = [
    ":number",
    ":number useGrouping=always",
    ":number useGrouping=min2",
    ":number useGrouping=never",
    ":number minimumFractionDigits=2",
    ":number maximumSignificantDigits=3",
    ":number signDisplay=always",
    ":integer",
    ":percent",
    ":percent maximumFractionDigits=1",
    ":currency currency=EUR",
    ":currency currency=USD",
    ":currency currency=JPY",
    ":currency currency=EUR currencyDisplay=code",
    ":currency currency=EUR currencyDisplay=name",
    ":currency currency=USD currencyDisplay=narrowSymbol",
    ":currency currency=EUR currencySign=accounting",
    ":currency currency=EUR fractionDigits=0",
    ":unit unit=kilometer",
    ":unit unit=kilometer unitDisplay=long",
    ":unit unit=kilometer-per-hour unitDisplay=narrow",
    ":unit unit=celsius",
    ":unit unit=kilogram unitDisplay=long",
    ":unit unit=liter unitDisplay=long",
];

const LOC_SELECTS: [(&str, &str); 2] = [
    ("select cardinal", ""),
    ("select ordinal", " select=ordinal"),
];

/// Every case: `(locale, function, value, source)`, catalog = locale.
fn loc_cases() -> Vec<(&'static str, String, &'static str, String)> {
    let mut cases = Vec::new();
    for locale in PANEL {
        for f in LOC_FUNCS {
            for v in LOC_VALUES {
                cases.push((locale, f.to_owned(), v, format!("{{{} {f}}}", quoted(v))));
            }
        }
        for (name, opt) in LOC_SELECTS {
            for v in LOC_VALUES {
                cases.push((
                    locale,
                    name.to_owned(),
                    v,
                    format!(
                        ".local $n = {{{} :number{opt}}} .match $n zero {{{{zero}}}} one {{{{one}}}} two {{{{two}}}} few {{{{few}}}} many {{{{many}}}} * {{{{other}}}}",
                        quoted(v)
                    ),
                ));
            }
        }
    }
    cases
}

pub(crate) fn loc(out: &Path) -> Result<()> {
    let cases = loc_cases();
    let mut blob = Blob::default();
    let mut index = Vec::new();
    for locale in PANEL {
        let mine: Vec<&(&str, String, &str, String)> =
            cases.iter().filter(|c| c.0 == locale).collect();
        let sources: Vec<String> = mine.iter().map(|c| c.3.clone()).collect();
        let cat = blob.push(&multi(&sources, locale)?);
        for (id, c) in mine.iter().enumerate() {
            index.push(
                json!({"locale": c.0, "fn": c.1, "v": c.2, "src": c.3, "cat": cat, "id": id}),
            );
        }
    }
    blob.write(out, "loc", json!({"cases": index}))
}

/// The locale-symbol cases through the Rust registry (`FUNCTIONS` above):
/// `loc-rust.json` = `{"registry": …, "out": [[text, errors], …]}` in case
/// order, what `web/lib/compare.mjs` takes like an engine's output.
pub(crate) fn loc_format(out: &Path) -> Result<()> {
    let cases = loc_cases();
    let mut rows = Vec::with_capacity(cases.len());
    for (locale, _, _, src) in &cases {
        let c = mf2::compile_str(src, locale)?;
        let f = Formatter::new(&c.catalog, &REGISTRY, &CX);
        let mut text = String::new();
        let mut errs = Vec::new();
        f.write(mf2::Compiled::ID, &[] as &[Arg<'_>], &mut text, &mut errs);
        rows.push(json!([text, error_names(&errs)]));
    }
    std::fs::create_dir_all(out).map_err(io(out))?;
    let path = out.join("loc-rust.json");
    let names: Vec<&str> = FUNCTIONS.iter().map(|(n, _)| *n).collect();
    let engine = if cfg!(feature = "fn-number") {
        "rust (native, mf2-fn-number)"
    } else {
        "rust (native, core: neutral digits)"
    };
    let doc = json!({"engine": engine, "registry": names, "out": rows});
    std::fs::write(&path, doc.to_string()).map_err(io(&path))?;
    eprintln!("{}: {} cases", path.display(), cases.len());
    Ok(())
}

// ──────────────────────────────────────────────────────────── edges ──

/// `:integer`'s rounding, `:offset`'s arithmetic and exact-key selection —
/// what P0.5's corpus (`:number` only) does not reach — as messages without
/// arguments, compared `rust` vs `intl` (`web/lib/edge.mjs`).
#[allow(clippy::items_after_statements)]
pub(crate) fn edge_sources() -> Vec<String> {
    const VALUES: [&str; 14] = [
        "-2.5",
        "-1.5",
        "-0.5",
        "-0.4",
        "-0",
        "0.4",
        "0.5",
        "1.5",
        "2.5",
        "1.25",
        "9.999",
        "1e3",
        "0.42e+1",
        "123456789012345678901234567890.5",
    ];
    const MODES: [&str; 10] = [
        "",
        "ceil",
        "floor",
        "expand",
        "trunc",
        "halfCeil",
        "halfFloor",
        "halfExpand",
        "halfTrunc",
        "halfEven",
    ];
    let mut out = Vec::new();
    for v in VALUES {
        for m in MODES {
            let opt = if m.is_empty() {
                String::new()
            } else {
                format!(" roundingMode={m}")
            };
            out.push(format!("{{{} :integer{opt}}}", quoted(v)));
        }
        for o in [
            " signDisplay=always",
            " signDisplay=exceptZero",
            " minimumIntegerDigits=3",
            " maximumSignificantDigits=1",
        ] {
            out.push(format!("{{{} :integer{o}}}", quoted(v)));
        }
        out.push(format!(
            ".local $x = {{{} :integer}} {{{{{{$x :number minimumFractionDigits=2}}}}}}",
            quoted(v)
        ));
    }
    const OFFSET: [&str; 18] = [
        "0",
        "-0",
        "1",
        "-1",
        "5",
        "-5",
        "1.5",
        "-1.5",
        "0.25",
        "-0.25",
        "99.99",
        "-100.01",
        "1234567890123456789012345678901234567890",
        "1e30",
        "1e-30",
        "0.001",
        "-98.5",
        "12e-1",
    ];
    for v in OFFSET {
        for n in [0, 1, 2, 5, 10, 99] {
            for op in ["add", "subtract"] {
                out.push(format!("{{{} :offset {op}={n}}}", quoted(v)));
            }
        }
    }
    for v in ["1", "-3", "2.5"] {
        out.push(format!(
            ".local $x = {{{} :integer signDisplay=always}} .local $y = {{$x :offset subtract=3}} {{{{{{$y}} {{$y :number}}}}}}",
            quoted(v)
        ));
    }
    for v in ["1", "1.0", "1.5", "0.99", "10", "-1", "0"] {
        for o in [
            "",
            " minimumFractionDigits=1",
            " maximumSignificantDigits=2",
            " maximumFractionDigits=0",
            " select=exact",
            " select=ordinal",
            " minimumIntegerDigits=2",
        ] {
            out.push(format!(
                ".local $n = {{{} :number{o}}} .match $n 0 {{{{=0}}}} 1 {{{{=1}}}} 1.0 {{{{=1.0}}}} 01 {{{{=01}}}} 1.5 {{{{=1.5}}}} one {{{{one}}}} two {{{{two}}}} few {{{{few}}}} * {{{{other}}}}",
                quoted(v)
            ));
        }
    }
    out
}

pub(crate) fn edge(out: &Path) -> Result<()> {
    let sources = edge_sources();
    let mut blob = Blob::default();
    blob.push(&multi(&sources, "en")?);
    blob.write(out, "edge", json!({"sources": sources}))
}

// ─────────────────────────────────────────────────────── catalog data ──

/// What each panel locale's catalog carries for numbers today and the
/// option would not need: `plural.cardinal` + `plural.ordinal` (02 §4.1;
/// `Intl.PluralRules` gives the category), `number.symbols` and
/// `number.patterns` for `:percent` and `:currency` (02 §4.2–§4.5; A2).
/// Currency and unit display data (A4) do not exist yet. The gzip figure is
/// a small catalog — the timed messages plus a `:percent` and a
/// `:currency` one — with and without those entries (`gzip -9 -n`).
#[cfg(feature = "number-data")]
pub(crate) fn catalog_data(out: &Path) -> Result<()> {
    use mf2_locale_data::{CurrencyNeeds, NumberNeeds, number_locale_entries};
    use std::process::Command;
    let mut sources: Vec<String> = SPEED.iter().map(|(_, s)| (*s).to_owned()).collect();
    sources.push("{$n :percent}".to_owned());
    sources.push("{$n :currency currency=EUR}".to_owned());
    let gz = |bytes: &[u8]| -> Result<usize> {
        let mut child = Command::new("gzip")
            .args(["-9", "-n", "-c"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(io("gzip"))?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write as _;
            stdin.write_all(bytes).map_err(io("gzip"))?;
        }
        let o = child.wait_with_output().map_err(io("gzip"))?;
        Ok(o.stdout.len())
    };
    let mut md = String::from(
        "| locale | plural (card + ord) B | number.symbols B | number.patterns (percent, currency) B | total B | catalog gz with | without | **Δ gz** |\n|---|---:|---:|---:|---:|---:|---:|---:|\n",
    );
    for locale in PANEL {
        let plural = mf2_locale_data::plural_locale_entries(
            locale,
            &[PluralKind::Cardinal, PluralKind::Ordinal],
        )?;
        let mut needs = NumberNeeds::default();
        needs.symbols = true;
        needs.percent = true;
        needs.currency = Some(CurrencyNeeds::ALL);
        let number = number_locale_entries(locale, &needs)?;
        let p: usize = plural.iter().map(|(_, b)| b.len()).sum();
        let sym: usize = number
            .iter()
            .filter(|(k, _)| *k == mf2_catalog::format::locale_key::NUMBER_SYMBOLS)
            .map(|(_, b)| b.len())
            .sum();
        let pat: usize = number
            .iter()
            .filter(|(k, _)| *k == mf2_catalog::format::locale_key::NUMBER_PATTERNS)
            .map(|(_, b)| b.len())
            .sum();
        let mut all = plural.clone();
        all.extend(number);
        all.sort_by_key(|(k, _)| *k);
        let with = gz(&crate::catalogs::multi_with(&sources, locale, all)?.bytes)?;
        let without = gz(&crate::catalogs::multi_with(&sources, locale, Vec::new())?.bytes)?;
        let _ = writeln!(
            md,
            "| {locale} | {p} | {sym} | {pat} | {} | {with} | {without} | **{}** |",
            p + sym + pat,
            with.saturating_sub(without)
        );
    }
    std::fs::create_dir_all(out).map_err(io(out))?;
    let path = out.join("catalog-data.md");
    std::fs::write(&path, &md).map_err(io(&path))?;
    print!("{md}");
    Ok(())
}

// ─────────────────────────────────────────────────────── observations ──

/// Native facts the report cites: `:offset` with a zero operand or offset.
pub(crate) fn observe() -> Result<()> {
    for src in [
        "{0 :offset subtract=5}",
        "{0 :offset add=5}",
        "{-5 :offset add=0}",
        "{5 :offset subtract=0}",
        "{-0 :offset add=0}",
        "{-0 :offset subtract=0}",
        "{-3 :offset add=3}",
        "{-2 :offset add=5}",
        "{1.5 :offset subtract=2}",
    ] {
        let c = mf2::compile_str(src, "en")?;
        let f = Formatter::new(&c.catalog, &REGISTRY, &CX);
        let mut text = String::new();
        let mut errs = Vec::new();
        f.write(mf2::Compiled::ID, &[] as &[Arg<'_>], &mut text, &mut errs);
        println!("{src}\t{text}\t{}", error_names(&errs));
    }
    Ok(())
}

/// The number split (`plan/08` §6, task 18.4): per panel locale, a catalog of
/// the locale-symbol panel's `:currency` and `:unit` messages with every
/// LOCALE entry their corpus needs (`NumberNeeds::add_message`, the build's
/// slicing rule), and the same without `currency.data` and `unit.data`,
/// which the split's client does not read: the two entries' raw bytes and
/// the catalog's brotli bytes (quality 11, window 22, as `mf2-build` writes
/// `.br`). Writes `names-data.md` and `names-data.tsv`.
#[cfg(feature = "number-data")]
pub(crate) fn names_data(out: &Path) -> Result<()> {
    use mf2_catalog::format::locale_key;
    use mf2_locale_data::{LocaleNeeds, NumberNeeds, locale_entries};
    let br = |bytes: &[u8]| -> Result<usize> {
        use std::io::Write as _;
        let mut o = Vec::new();
        {
            let mut w = brotli::CompressorWriter::new(&mut o, 4096, 11, 22);
            w.write_all(bytes).map_err(io("brotli"))?;
            w.flush().map_err(io("brotli"))?;
        }
        Ok(o.len())
    };
    let is_name = |k: u32| k == locale_key::CURRENCY_DATA || k == locale_key::UNIT_DATA;
    let cases = loc_cases();
    let mut md = String::from(
        "| locale | currency.data B | unit.data B | catalog br with | without | **Δ br** |\n|---|---:|---:|---:|---:|---:|\n",
    );
    let mut tsv = String::from("locale\tcurrency_raw\tunit_raw\tbr_with\tbr_without\n");
    for locale in PANEL {
        let sources: Vec<String> = cases
            .iter()
            .filter(|c| c.0 == locale && (c.1.starts_with(":currency") || c.1.starts_with(":unit")))
            .map(|c| c.3.clone())
            .collect();
        let mut numbers = NumberNeeds::default();
        numbers.symbols = true;
        for src in &sources {
            if let Some(m) = &mf2_syntax::parse_model(src).message {
                numbers.add_message(m);
            }
        }
        let mut needs = LocaleNeeds::default();
        needs.numbers = numbers;
        let all = locale_entries(locale, &needs)?;
        let raw = |key: u32| -> usize {
            all.iter()
                .filter(|(k, _)| *k == key)
                .map(|(_, b)| b.len())
                .sum()
        };
        let (currency, unit) = (raw(locale_key::CURRENCY_DATA), raw(locale_key::UNIT_DATA));
        let kept: Vec<(u32, Vec<u8>)> = all.iter().filter(|(k, _)| !is_name(*k)).cloned().collect();
        let with = br(&crate::catalogs::multi_with(&sources, locale, all)?.bytes)?;
        let without = br(&crate::catalogs::multi_with(&sources, locale, kept)?.bytes)?;
        let _ = writeln!(
            md,
            "| {locale} | {currency} | {unit} | {with} | {without} | **{}** |",
            with.saturating_sub(without)
        );
        let _ = writeln!(tsv, "{locale}\t{currency}\t{unit}\t{with}\t{without}");
    }
    std::fs::create_dir_all(out).map_err(io(out))?;
    for (name, text) in [("names-data.md", &md), ("names-data.tsv", &tsv)] {
        let path = out.join(name);
        std::fs::write(&path, text).map_err(io(&path))?;
    }
    print!("{md}");
    Ok(())
}
