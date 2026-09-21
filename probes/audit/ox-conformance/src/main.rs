use ox_mf2_parser::{build_semantic_model, parse_source, validate_semantics, ParseOptions, SourceFileInput, SourceStore};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};

const DM: &[&str] = &["duplicate-declaration","duplicate-option-name","duplicate-variant","missing-fallback-variant","missing-selector-annotation","variant-key-mismatch"];

fn walk(d: &Path, out: &mut Vec<PathBuf>) { for e in fs::read_dir(d).unwrap() { let p = e.unwrap().path(); if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "json") { out.push(p) } } }

fn map(code: &str) -> &str { match code { "variant-key-arity-mismatch" => "variant-key-mismatch", "invalid-declaration-dependency" => "duplicate-declaration", c => c } }

fn main() {
    let root = std::env::args().nth(1).unwrap();
    let mut files = vec![]; walk(Path::new(&root), &mut files); files.sort();
    let (mut total, mut fail) = (0, 0);
    for f in files {
        let suite: Value = serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        let defaults = suite.get("defaultTestProperties").cloned().unwrap_or(Value::Null);
        let (mut ft, mut ff) = (0, 0);
        for (i, t) in suite["tests"].as_array().unwrap().iter().enumerate() {
            let Some(src) = t.get("src").or_else(|| defaults.get("src")).and_then(Value::as_str) else { continue };
            let exp = t.get("expErrors").or_else(|| defaults.get("expErrors"));
            let exp_types: BTreeSet<String> = exp.and_then(Value::as_array).map(|a| a.iter().filter_map(|e| e.get("type").and_then(Value::as_str).map(str::to_owned)).collect()).unwrap_or_default();
            let want: BTreeSet<&str> = exp_types.iter().map(String::as_str).filter(|t| *t == "syntax-error" || DM.contains(t)).collect();
            let mut sources = SourceStore::new();
            let id = sources.add(SourceFileInput { source: src, ..Default::default() });
            let res = parse_source(&sources, id, ParseOptions::default()).unwrap();
            let mut got: BTreeSet<String> = BTreeSet::new();
            if !res.diagnostics.is_empty() { got.insert("syntax-error".into()); } else {
                match build_semantic_model(&sources, &res).and_then(|m| validate_semantics(&m)) {
                    Ok(ds) => for d in ds { got.insert(map(d.code().json_code()).to_owned()); },
                    Err(e) => { got.insert(format!("INVARIANT:{e}")); }
                }
            }
            let got_ref: BTreeSet<&str> = got.iter().map(String::as_str).collect();
            ft += 1;
            if got_ref != want { ff += 1; println!("MISMATCH {}#{i} want={want:?} got={got_ref:?} src={src:?}", f.file_name().unwrap().to_string_lossy()); }
        }
        println!("{:<28} {ft:>4} tests, {ff} mismatches", f.strip_prefix(&root).unwrap().display());
        total += ft; fail += ff;
    }
    println!("TOTAL {total} tests, {fail} mismatches (syntax + data-model error classification only)");
}
