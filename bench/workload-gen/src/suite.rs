//! `bench/corpora/suite.json`: the `src` of every WG test, in file-path then
//! index order.

use std::path::Path;

use crate::error::Error;
use crate::json;

/// One suite test's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Path relative to `test/tests`, `/`-separated.
    pub file: String,
    /// Index in the file's `tests` array.
    pub index: usize,
    /// The message source (`src`, or the file's `defaultTestProperties.src`).
    pub src: String,
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, base, out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            let rel = path
                .strip_prefix(base)
                .map_err(|_| Error::NotFound(path.clone()))?;
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
        }
    }
    Ok(())
}

/// Reads every test of the suite under `tests_dir`.
pub fn entries(tests_dir: &Path) -> Result<Vec<Entry>, Error> {
    if !tests_dir.is_dir() {
        return Err(Error::NotFound(tests_dir.to_path_buf()));
    }
    let mut files = Vec::new();
    collect(tests_dir, tests_dir, &mut files)?;
    files.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut out = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(tests_dir.join(&file))?;
        let doc: serde_json::Value = serde_json::from_str(&text)?;
        let default_src = doc
            .get("defaultTestProperties")
            .and_then(|d| d.get("src"))
            .and_then(serde_json::Value::as_str);
        let tests = doc
            .get("tests")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| Error::Suite {
                file: file.clone(),
                message: "no `tests` array".into(),
            })?;
        for (index, test) in tests.iter().enumerate() {
            let src = test
                .get("src")
                .and_then(serde_json::Value::as_str)
                .or(default_src)
                .ok_or_else(|| Error::Suite {
                    file: file.clone(),
                    message: format!("test {index} has no `src`"),
                })?;
            out.push(Entry {
                file: file.clone(),
                index,
                src: src.to_owned(),
            });
        }
    }
    Ok(out)
}

/// The JSON array, one entry per line.
pub fn to_json(entries: &[Entry]) -> String {
    let mut out = String::from("[\n");
    for (i, e) in entries.iter().enumerate() {
        out.push_str("  {\"file\": ");
        json::string(&mut out, &e.file);
        out.push_str(", \"index\": ");
        out.push_str(&e.index.to_string());
        out.push_str(", \"src\": ");
        json::string(&mut out, &e.src);
        out.push('}');
        out.push_str(if i + 1 == entries.len() { "\n" } else { ",\n" });
    }
    out.push_str("]\n");
    out
}
