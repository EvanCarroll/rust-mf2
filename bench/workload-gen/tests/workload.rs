//! Integration tests for the workload generator: determinism, shape, scale,
//! `.mf2` ↔ JSON consistency, message well-formedness, corpora freshness,
//! templates and canaries; the same workload as Fluent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use workload_gen::model::canary;
use workload_gen::{Format, Knobs, SitePlan, Template, Workload, suite};

fn builtins() -> Vec<Template> {
    ["literal", "closure"]
        .iter()
        .map(|n| Template::builtin(n).unwrap())
        .collect()
}

fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    dir
}

fn read_tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn same_seed_gives_byte_identical_output() {
    let knobs = Knobs::default();
    let a = workload_gen::generate(&knobs, &builtins()).unwrap();
    let b = workload_gen::generate(&knobs, &builtins()).unwrap();
    assert_eq!(a, b, "in-memory output differs between two runs");
    // 4 locales × (18 files + 1 JSON) + sites.json + 2 apps × (60 component
    // modules + 9 other files).
    assert_eq!(a.len(), 4 * 19 + 1 + 2 * (60 + 9));

    let (da, db) = (tmp("det-a"), tmp("det-b"));
    a.write_to(&da, &knobs.summary()).unwrap();
    b.write_to(&db, &knobs.summary()).unwrap();
    let (ta, tb) = (read_tree(&da), read_tree(&db));
    assert_eq!(ta.len(), a.len() + 1, "marker + every file on disk");
    assert_eq!(ta, tb, "on-disk trees differ");
    // Rewriting into the same directory is idempotent.
    a.write_to(&da, &knobs.summary()).unwrap();
    assert_eq!(read_tree(&da), tb);

    let other = Knobs {
        seed: 2,
        ..Knobs::default()
    };
    let c = workload_gen::generate(&other, &builtins()).unwrap();
    assert_ne!(
        a.get("json/en.json"),
        c.get("json/en.json"),
        "seed has no effect"
    );
}

#[test]
fn shape_is_within_tolerance() {
    let report = workload_gen::report(&Knobs::default()).unwrap();
    println!("{}", report.render());
    assert!(report.passed(), "{:?}", report.failures());
}

#[test]
fn ten_times_scale() {
    let knobs = Knobs {
        messages: 16_000,
        sites: 18_600,
        ..Knobs::default()
    };
    let report = workload_gen::report(&knobs).unwrap();
    println!("{}", report.render());
    assert!(report.passed(), "{:?}", report.failures());
    let files = workload_gen::generate(&knobs, &[Template::builtin("literal").unwrap()]).unwrap();
    let en: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(files.get("json/en.json").unwrap()).unwrap();
    assert_eq!(en.len(), 16_000);
}

#[test]
fn function_knobs_add_number_and_datetime() {
    let knobs = Knobs {
        number: 20,
        datetime: 10,
        ..Knobs::default()
    };
    let wl = Workload::generate(&knobs).unwrap();
    let json = workload_gen::corpus_json(&wl).unwrap();
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&json).unwrap();
    let count = |needle: &str| {
        map.values()
            .filter(|v| v.as_str().unwrap().contains(needle))
            .count()
    };
    assert_eq!(count(":number"), 20);
    assert_eq!(count(":datetime"), 10);
    let report = workload_gen::report(&knobs).unwrap();
    assert!(report.passed(), "{:?}", report.failures());
}

/// A reader for the working grammar of plans/05-tooling.md §2, as far as the
/// generator uses it. Returns the locale and `(full id, value)` pairs.
fn parse_mf2(src: &str) -> (String, Vec<(String, String)>) {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut locale = String::new();
    let mut in_front = true;
    let mut section: Option<String> = None;
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        assert!(
            !line.starts_with([' ', '\t']),
            "stray continuation line: {line:?}"
        );
        if in_front {
            if line == "---" {
                in_front = false;
            } else if let Some(tag) = line.strip_prefix("@locale ") {
                tag.clone_into(&mut locale);
            }
            continue;
        }
        if line.starts_with('@') {
            continue;
        }
        if let Some(head) = line.strip_prefix('[') {
            section = Some(head.strip_suffix(']').expect("section head").to_owned());
            continue;
        }
        let (key, rest) = line.split_once('=').expect("entry");
        let mut parts = vec![rest.trim_start().to_owned()];
        while i < lines.len() && lines[i].starts_with([' ', '\t']) {
            parts.push(lines[i].trim_start().to_owned());
            i += 1;
        }
        let mut value = String::new();
        let mut pending_lf = false;
        for (k, part) in parts.iter().enumerate() {
            if k == 0 && part.is_empty() {
                continue; // `key =` then the value on continuation lines
            }
            if pending_lf {
                value.push('\n');
            }
            let backslashes = part.len() - part.trim_end_matches('\\').len();
            if backslashes % 2 == 1 {
                value.push_str(&part[..part.len() - 1]); // escaped line break
                pending_lf = false;
            } else {
                value.push_str(part);
                pending_lf = true;
            }
        }
        let key = key.trim_end();
        let id = section
            .as_ref()
            .map_or_else(|| key.to_owned(), |s| format!("{s}.{key}"));
        out.push((id, value));
    }
    (locale, out)
}

#[test]
fn mf2_files_match_flat_json() {
    let files = workload_gen::generate(&Knobs::default(), &[]).unwrap();
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        let json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(files.get(&format!("json/{tag}.json")).unwrap()).unwrap();
        let mut from_mf2: BTreeMap<String, String> = BTreeMap::new();
        let mut count = 0;
        for (path, bytes) in files.iter() {
            if !path.starts_with(&format!("locales/{tag}/")) {
                continue;
            }
            count += 1;
            let (locale, entries) = parse_mf2(std::str::from_utf8(bytes).unwrap());
            assert_eq!(locale, tag, "{path}");
            for (id, value) in entries {
                assert!(
                    from_mf2.insert(id.clone(), value).is_none(),
                    "duplicate id {id}"
                );
            }
        }
        assert_eq!(count, 18);
        assert_eq!(from_mf2.len(), json.len(), "{tag}");
        for (id, value) in &from_mf2 {
            assert_eq!(json[id].as_str(), Some(value.as_str()), "{tag} {id}");
        }
    }
}

/// Checks one pattern of the MF2 subset the generator writes and returns
/// the variables it references.
fn check_pattern(p: &str) -> Result<BTreeSet<&str>, String> {
    let mut vars = BTreeSet::new();
    let mut rest = p;
    while let Some(pos) = rest.find(['{', '}', '\\']) {
        let c = rest.as_bytes()[pos];
        if c == b'\\' {
            rest = &rest[pos + 2..];
            continue;
        }
        if c == b'}' {
            return Err(format!("stray `}}` in {p:?}"));
        }
        let end = rest[pos..]
            .find('}')
            .ok_or(format!("unclosed `{{` in {p:?}"))?
            + pos;
        let inner = &rest[pos + 1..end];
        if inner.contains('{') {
            return Err(format!("nested `{{` in {p:?}"));
        }
        if let Some(v) = inner.strip_prefix('$') {
            let name = v.split(' ').next().unwrap();
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(format!("bad variable in {p:?}"));
            }
            let rest_expr = &v[name.len()..];
            if !(rest_expr.is_empty()
                || rest_expr.starts_with(" :number")
                || rest_expr.starts_with(" :datetime"))
            {
                return Err(format!("unexpected expression {inner:?}"));
            }
            vars.insert(name);
        } else if !(inner.starts_with('#') || inner.starts_with('/')) {
            return Err(format!("unexpected placeholder {inner:?}"));
        }
        rest = &rest[end + 1..];
    }
    Ok(vars)
}

fn check_message(src: &str) -> Result<(), String> {
    if src.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("control character in {src:?}"));
    }
    if !src.starts_with('.') {
        if src.starts_with(char::is_whitespace) || src.contains('\n') {
            return Err(format!("bad simple message {src:?}"));
        }
        return check_pattern(src).map(|_| ());
    }
    let lines: Vec<&str> = src.split('\n').collect();
    let var = lines[0]
        .strip_prefix(".input {$")
        .and_then(|l| l.strip_suffix(" :integer}"))
        .ok_or(format!("bad .input in {src:?}"))?;
    if lines.get(1) != Some(&format!(".match ${var}").as_str()) {
        return Err(format!("bad .match in {src:?}"));
    }
    let variants = &lines[2..];
    if variants.is_empty() || !variants.last().unwrap().starts_with("* {{") {
        return Err(format!("no catch-all variant in {src:?}"));
    }
    for v in variants {
        let (key, pattern) = v.split_once(' ').ok_or(format!("bad variant {v:?}"))?;
        if !["zero", "one", "two", "few", "many", "*"].contains(&key) {
            return Err(format!("bad key {key:?}"));
        }
        let inner = pattern
            .strip_prefix("{{")
            .and_then(|p| p.strip_suffix("}}"))
            .ok_or(format!("bad quoted pattern {pattern:?}"))?;
        let used = check_pattern(inner)?;
        if used.iter().any(|u| *u != var) {
            return Err(format!("undeclared variable in {v:?}"));
        }
    }
    Ok(())
}

#[test]
fn messages_are_well_formed_mf2() {
    let files = workload_gen::generate(&Knobs::default(), &[]).unwrap();
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        let json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(files.get(&format!("json/{tag}.json")).unwrap()).unwrap();
        for (id, src) in &json {
            if let Err(e) = check_message(src.as_str().unwrap()) {
                panic!("{tag} {id}: {e}");
            }
        }
    }
}

#[test]
fn translations_use_source_variables_and_own_plural_categories() {
    let files = workload_gen::generate(&Knobs::default(), &[]).unwrap();
    let load = |tag: &str| -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_slice(files.get(&format!("json/{tag}.json")).unwrap()).unwrap()
    };
    let en = load("en");
    let pl = load("pl");
    let vars = |s: &str| -> BTreeSet<String> {
        s.split('$')
            .skip(1)
            .map(|t| {
                t.chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect()
            })
            .collect()
    };
    let mut selects = 0;
    for (id, src) in &en {
        let (s, t) = (src.as_str().unwrap(), pl[id].as_str().unwrap());
        assert_eq!(vars(s), vars(t), "{id}");
        if s.contains(".match") {
            selects += 1;
            for key in ["\none {{", "\nfew {{", "\nmany {{", "\n* {{"] {
                assert!(t.contains(key), "pl {id} lacks {key:?}");
            }
        }
    }
    assert_eq!(selects, 14);
}

#[test]
fn canaries_are_present_and_referenced() {
    let knobs = Knobs::default();
    let files = workload_gen::generate(&knobs, &builtins()).unwrap();
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        let json = std::str::from_utf8(files.get(&format!("json/{tag}.json")).unwrap()).unwrap();
        assert!(json.contains(&canary::text(tag)), "{tag}");
        assert!(json.contains(canary::MESSAGE_ID));
        assert!(json.contains(canary::VARIABLE));
        // Canary strings occur nowhere else.
        assert_eq!(json.matches(canary::TEXT_PREFIX).count(), 1, "{tag}");
    }
    let wl = Workload::generate(&knobs).unwrap();
    let plan = SitePlan::generate(&wl).unwrap();
    assert!(wl.messages[plan.sites[0].message].canary);
    let closure = String::from_utf8(
        files
            .get("app-closure/src/components/c000.rs")
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(closure.contains(canary::MESSAGE_ID) && closure.contains(canary::VARIABLE));
}

#[test]
fn committed_corpora_are_current() {
    let root = workload_gen::repo_root();
    let wl = Workload::generate(&Knobs::default()).unwrap();
    let corpus = std::fs::read_to_string(root.join("bench/corpora/workload-1600.json")).unwrap();
    assert!(
        corpus == workload_gen::corpus_json(&wl).unwrap(),
        "bench/corpora/workload-1600.json is stale: run `cargo xtask gen-workload corpora`"
    );
    let entries = suite::entries(&root.join("third_party/message-format-wg/test/tests")).unwrap();
    assert_eq!(entries.len(), 462);
    assert!(
        entries
            .windows(2)
            .all(|w| { (w[0].file.as_bytes(), w[0].index) < (w[1].file.as_bytes(), w[1].index) })
    );
    let committed = std::fs::read_to_string(root.join("bench/corpora/suite.json")).unwrap();
    assert!(
        committed == suite::to_json(&entries),
        "bench/corpora/suite.json is stale"
    );
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&committed).unwrap();
    assert_eq!(parsed.len(), 462);
}

#[test]
fn custom_template_is_a_data_only_addition() {
    let dir = tmp("tpl-tr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("template.toml"),
        r#"
name = "tr"
description = "Test: one Copy description type from a probe crate."
dependencies = ['probe-tr = { path = "{{template_dir}}/crate" }']
prelude = "use probe_tr::{tr, tr_args, Arg};"
support = "support.rs"

[args]
plain = "Arg::from({{value}})"
signal = "Arg::reactive({{signal}})"

[deferred]
type = "probe_tr::Tr"
view = "{{label}}"
string = "{{label}}.to_string()"

[site.string]
none = "tr({{index}}).to_string()"
plain = "tr_args({{index}}, [{{args}}]).to_string()"

[site.child]
none = "tr({{index}})"
rich = "tr({{index}}) /* markup: {{markup}} */"
plain = "tr_args({{index}}, [{{args}}])"
get = "move || tr_args({{index}}, [{{args}}])"

[site.attr]
none = "tr({{index}})"
plain = "tr_args({{index}}, [{{args}}])"

[site.reactive_prop]
none = "tr({{index}})"
plain = "tr_args({{index}}, [{{args}}])"

[site.string_prop]
none = "tr({{index}})"
plain = "tr_args({{index}}, [{{args}}])"

[site.deferred]
none = "tr({{index}})"

[site.if_else]
none = "tr({{index}})"
plain = "tr_args({{index}}, [{{args}}])"
"#,
    )
    .unwrap();
    std::fs::write(dir.join("support.rs"), "//! probe support\n").unwrap();
    let template = Template::resolve(dir.to_str().unwrap()).unwrap();
    let files = workload_gen::generate(&Knobs::default(), &[template]).unwrap();
    let cargo = std::str::from_utf8(files.get("app-tr/Cargo.toml").unwrap()).unwrap();
    let canonical = std::fs::canonicalize(&dir).unwrap();
    assert!(cargo.contains(&format!(
        "probe-tr = {{ path = \"{}/crate\" }}",
        canonical.display()
    )));
    let all: String = files
        .iter()
        .filter(|(p, _)| p.starts_with("app-tr/src/components/"))
        .map(|(_, b)| String::from_utf8(b.to_vec()).unwrap())
        .collect();
    assert!(all.contains("Arg::reactive(count)") || all.contains("Arg::reactive(name)"));
    assert!(all.contains("move || tr_args("));
    assert!(all.contains("Arg::from(who)") || all.contains("Arg::from(n)"));
    assert!(!all.contains("{{"), "unexpanded placeholder");

    // An unknown placeholder is rejected when the template loads.
    let bad = tmp("tpl-bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(
        bad.join("template.toml"),
        "name = \"bad\"\ndescription = \"x\"\n[deferred]\ntype = \"u8\"\nview = \"x\"\nstring = \"x\"\n[site.child]\nnone = \"{{nope}}\"\n",
    )
    .unwrap();
    assert!(Template::resolve(bad.to_str().unwrap()).is_err());
}

// ---- Fluent (`--format ftl`, plans/16 A2) ----

fn ftl_files(knobs: &Knobs) -> workload_gen::Files {
    workload_gen::generate_as(knobs, &[Format::Ftl], &[]).unwrap()
}

#[test]
fn ftl_is_reproducible_and_measures_as_the_table() {
    let knobs = Knobs::default();
    let a = ftl_files(&knobs);
    assert_eq!(a, ftl_files(&knobs), "same seed, different bytes");
    // 4 locales × (18 files + 1 JSON); no `.mf2` asked for.
    assert_eq!(a.len(), 4 * 19);
    assert!(a.iter().all(|(p, _)| !p.starts_with("locales/")));
    let c = ftl_files(&Knobs {
        seed: 2,
        ..Knobs::default()
    });
    assert_ne!(a.get("ftl/en/chat.ftl"), c.get("ftl/en/chat.ftl"));

    // Asking for both formats changes neither.
    let both = workload_gen::generate_as(&knobs, &[Format::Mf2, Format::Ftl], &[]).unwrap();
    let mf2 = workload_gen::generate(&knobs, &[]).unwrap();
    for (path, bytes) in both.iter() {
        let alone = if path.starts_with("ftl/") { &a } else { &mf2 };
        assert_eq!(alone.get(path), Some(bytes), "{path}");
    }
    assert_eq!(both.len(), a.len() + mf2.len() - 4);

    // The shape, measured by parsing what was generated…
    let report = workload_gen::report_ftl(&knobs).unwrap();
    println!("{}", report.render());
    assert!(report.passed(), "{:?}", report.failures());
    // …and what was written.
    let dir = tmp("ftl-stats");
    a.write_to(&dir, &knobs.summary()).unwrap();
    let on_disk = workload_gen::report_ftl_dir(&knobs, &dir.join("ftl")).unwrap();
    assert!(on_disk.passed(), "{:?}", on_disk.failures());
    // The same rows, locales in directory order.
    let rows = |r: &workload_gen::stats::Report| -> BTreeSet<String> {
        r.rows
            .iter()
            .take_while(|r| r.label != "call sites")
            .map(|r| format!("{} {} {}", r.label, r.target, r.actual))
            .collect()
    };
    assert_eq!(rows(&on_disk), rows(&report));
}

#[test]
fn ftl_stats_refuse_what_does_not_parse() {
    // Negative controls: an unclosed placeable and a stray line are errors
    // or `Junk`, and the measurement refuses them.
    for bad in ["a = { $x\n", "a = ok\n}}}\n", "a =\n"] {
        let err = workload_gen::stats::parse_ftl("bad.ftl", bad).unwrap_err();
        assert!(err.to_string().contains("bad.ftl"), "{err}");
    }
    let knobs = Knobs::default();
    let dir = tmp("ftl-junk");
    ftl_files(&knobs).write_to(&dir, &knobs.summary()).unwrap();
    let path = dir.join("ftl/pl/chat.ftl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("broken = { $\n");
    std::fs::write(&path, text).unwrap();
    let err = workload_gen::report_ftl_dir(&knobs, &dir.join("ftl")).unwrap_err();
    assert!(err.to_string().contains("pl/chat.ftl"), "{err}");
}

/// A Fluent pattern as the MF2 text it stands for: text, and `{$name}` for a
/// variable reference; `{ "" }` is empty.
fn ftl_text(pattern: &fluent_syntax::ast::Pattern<&str>) -> String {
    use fluent_syntax::ast::{Expression, InlineExpression, PatternElement};
    let mut out = String::new();
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => out.push_str(value),
            PatternElement::Placeable {
                expression: Expression::Inline(InlineExpression::VariableReference { id }),
            } => {
                out.push_str("{$");
                out.push_str(id.name);
                out.push('}');
            }
            PatternElement::Placeable {
                expression: Expression::Inline(InlineExpression::StringLiteral { value: "" }),
            } => {}
            other @ PatternElement::Placeable { .. } => panic!("unexpected {other:?}"),
        }
    }
    out
}

#[test]
fn ftl_says_what_the_mf2_says() {
    use fluent_syntax::ast::{Entry, Expression, PatternElement, VariantKey};
    use workload_gen::model::{Body, render_pattern};
    use workload_gen::{fluent, locale};

    let knobs = Knobs::default();
    let wl = Workload::generate(&knobs).unwrap();
    let files = ftl_files(&knobs);
    let ids = fluent::ids(&wl).unwrap();
    for loc in locale::locales(&knobs).unwrap() {
        let bodies = locale::bodies(&wl, &loc);
        let texts: Vec<(String, String)> = files
            .iter()
            .filter(|(p, _)| p.starts_with(&format!("ftl/{}/", loc.tag)))
            .map(|(p, b)| (p.to_owned(), String::from_utf8(b.to_vec()).unwrap()))
            .collect();
        assert_eq!(texts.len(), 18);
        let mut found = BTreeMap::new();
        for (path, text) in &texts {
            let resource = fluent_syntax::parser::parse(text.as_str()).unwrap();
            for entry in resource.body {
                match entry {
                    Entry::Message(m) => {
                        assert!(found.insert(m.id.name, (path.clone(), m)).is_none());
                    }
                    Entry::Junk { content } => panic!("{path}: junk {content:?}"),
                    _ => {}
                }
            }
        }
        assert_eq!(found.len(), wl.messages.len(), "{}", loc.tag);
        for (j, message) in wl.messages.iter().enumerate() {
            let (path, m) = &found[ids[j].as_str()];
            let what = format!("{} {}", loc.tag, message.id);
            // The file of the same name, the id with `-` for `.`.
            let ns = wl.files[message.file].namespace;
            assert_eq!(path, &format!("ftl/{}/{ns}.ftl", loc.tag), "{what}");
            assert_eq!(ids[j], message.id.replace('.', "-"));
            // Every argument in the comment's `Variables:` block.
            let comment = m.comment.as_ref().map(|c| c.content.join("\n"));
            for var in &message.vars {
                let line = format!("  ${} (", var.name);
                assert!(
                    comment.as_ref().is_some_and(|c| c.contains(&line)),
                    "{what}"
                );
            }
            let render = |p: &[workload_gen::model::Part]| {
                let mut s = String::new();
                render_pattern(p, &message.vars, &mut s);
                s
            };
            match &bodies[j] {
                Body::Pattern(p) if !message.markup.is_empty() => {
                    // Split around the element: before, element, after.
                    assert!(m.value.is_none(), "{what}");
                    let names: Vec<&str> = m.attributes.iter().map(|a| a.id.name).collect();
                    assert_eq!(names, ["before", message.markup[0], "after"], "{what}");
                    let at = p
                        .iter()
                        .position(|x| matches!(x, workload_gen::model::Part::Markup { .. }))
                        .unwrap();
                    let workload_gen::model::Part::Markup { inner, .. } = &p[at] else {
                        unreachable!()
                    };
                    let parts = [
                        render(&p[..at]).trim_end().to_owned(),
                        inner.clone(),
                        render(&p[at + 1..]).trim_start().to_owned(),
                    ];
                    for (attr, want) in m.attributes.iter().zip(&parts) {
                        assert_eq!(&ftl_text(&attr.value), want, "{what}");
                    }
                }
                Body::Pattern(p) => {
                    assert!(m.attributes.is_empty(), "{what}");
                    assert_eq!(ftl_text(m.value.as_ref().unwrap()), render(p), "{what}");
                }
                Body::Select { selector, variants } => {
                    let value = m.value.as_ref().unwrap();
                    let [
                        PatternElement::Placeable {
                            expression:
                                Expression::Select {
                                    selector: s,
                                    variants: vs,
                                },
                        },
                    ] = value.elements.as_slice()
                    else {
                        panic!("{what}: not a lone select");
                    };
                    assert_eq!(
                        s,
                        &fluent_syntax::ast::InlineExpression::VariableReference {
                            id: fluent_syntax::ast::Identifier {
                                name: message.vars[*selector].name
                            }
                        },
                        "{what}"
                    );
                    assert_eq!(vs.len(), variants.len(), "{what}");
                    for (v, (key, pattern)) in vs.iter().zip(variants) {
                        let VariantKey::Identifier { name } = v.key else {
                            panic!("{what}: a number key");
                        };
                        let want = if key == "*" { "other" } else { key.as_str() };
                        assert_eq!((name, v.default), (want, key == "*"), "{what}");
                        assert_eq!(ftl_text(&v.value), render(pattern), "{what}");
                    }
                }
            }
        }
    }
}
