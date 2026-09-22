//! Loading the vendored WG test suite (`third_party/message-format-wg/test/tests`).
//!
//! Every `*.json` file below the directory is a suite file; files are taken in
//! byte order of their `/`-separated relative paths, tests in file order. A
//! test's properties are its file's `defaultTestProperties` overlaid with the
//! test's own (a property present on the test replaces the default wholesale).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::key::{TestKey, canonical_json, key_hash};
use crate::matrix::TestKind;

/// One test, with defaults applied.
#[derive(Debug, Clone)]
pub struct SuiteTest {
    pub key: TestKey,
    /// 0-based position in its file (a hint in the ledger, not part of the key).
    pub index: usize,
    pub kind: TestKind,
    pub src: String,
    pub locale: String,
    pub params: Option<Value>,
    pub bidi_isolation: Option<String>,
    pub tags: Vec<String>,
    pub exp: Option<String>,
    pub exp_parts: Option<Value>,
    /// The `type` of each expected error.
    pub exp_errors: Vec<String>,
    pub description: Option<String>,
    /// Canonical JSON of all resolved properties (used to detect changed tests).
    pub resolved: String,
}

/// The whole suite.
#[derive(Debug, Clone)]
pub struct Suite {
    files: Vec<(String, usize)>,
    tests: Vec<SuiteTest>,
}

impl Suite {
    /// Loads every `*.json` file below `dir` (normally `…/test/tests`).
    pub fn load(dir: &Path) -> Result<Self> {
        let mut sources = Vec::new();
        collect(dir, "", &mut sources)?;
        Self::from_sources(sources)
    }

    /// Loads the vendored suite below `suite_dir` and our own tests below
    /// `extra_dir` (plans/01-conformance.md §5: gaps get tests written in the
    /// WG schema under `conformance/extra/`, run through the same layers),
    /// the latter under the relative path `extra/…`. A missing `extra_dir`
    /// adds nothing.
    pub fn load_with_extra(suite_dir: &Path, extra_dir: &Path) -> Result<Self> {
        let mut sources = Vec::new();
        collect(suite_dir, "", &mut sources)?;
        if extra_dir.is_dir() {
            collect(extra_dir, "extra", &mut sources)?;
        }
        Self::from_sources(sources)
    }

    /// Builds the suite from `(relative path, JSON text)` pairs, in any order.
    pub fn from_sources<I, P, T>(sources: I) -> Result<Self>
    where
        I: IntoIterator<Item = (P, T)>,
        P: Into<String>,
        T: AsRef<str>,
    {
        let mut parsed: Vec<(String, Value)> = Vec::new();
        for (path, text) in sources {
            let path = path.into();
            let value: Value =
                serde_json::from_str(text.as_ref()).map_err(|source| Error::Json {
                    file: path.clone(),
                    source,
                })?;
            parsed.push((path, value));
        }
        Self::from_values(parsed)
    }

    /// Builds the suite from parsed `(relative path, file JSON)` pairs.
    pub fn from_values(mut files: Vec<(String, Value)>) -> Result<Self> {
        files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let mut suite = Self {
            files: Vec::new(),
            tests: Vec::new(),
        };
        for (file, value) in files {
            let tests = load_file(&file, &value)?;
            suite.files.push((file, tests.len()));
            suite.tests.extend(tests);
        }
        Ok(suite)
    }

    /// All tests: files in path order, tests in file order.
    pub fn tests(&self) -> &[SuiteTest] {
        &self.tests
    }

    /// `(relative path, number of tests)` of every file, in path order.
    pub fn files(&self) -> &[(String, usize)] {
        &self.files
    }
}

/// Every `*.json` file below `dir`, as `(relative path, text)`; the path is
/// prefixed with `prefix/` when `prefix` is not empty.
fn collect(dir: &Path, prefix: &str, sources: &mut Vec<(String, String)>) -> Result<()> {
    let mut stack = vec![(dir.to_path_buf(), prefix.to_owned())];
    while let Some((path, rel)) = stack.pop() {
        let entries = fs::read_dir(&path).map_err(|source| Error::IoAt {
            path: path.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::IoAt {
                path: path.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let child = entry.path();
            if child.is_dir() {
                stack.push((child, child_rel));
            } else if Path::new(&name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                let text = fs::read_to_string(&child).map_err(|source| Error::IoAt {
                    path: child.clone(),
                    source,
                })?;
                sources.push((child_rel, text));
            }
        }
    }
    Ok(())
}

fn bad(file: &str, index: usize, message: impl Into<String>) -> Error {
    Error::SuiteTest {
        file: file.to_owned(),
        index,
        message: message.into(),
    }
}

fn load_file(file: &str, value: &Value) -> Result<Vec<SuiteTest>> {
    let file_err = |message: &str| Error::SuiteFile {
        file: file.to_owned(),
        message: message.to_owned(),
    };
    let obj = value
        .as_object()
        .ok_or_else(|| file_err("not a JSON object"))?;
    let defaults = match obj.get("defaultTestProperties") {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Err(file_err("`defaultTestProperties` is not an object")),
    };
    let tests = obj
        .get("tests")
        .and_then(Value::as_array)
        .ok_or_else(|| file_err("`tests` is missing or not an array"))?;

    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::with_capacity(tests.len());
    for (index, test) in tests.iter().enumerate() {
        let own = test
            .as_object()
            .ok_or_else(|| bad(file, index, "test is not an object"))?;
        let mut props = defaults.clone();
        for (k, v) in own {
            props.insert(k.clone(), v.clone());
        }
        let t = resolve(file, index, props, &mut seen)?;
        out.push(t);
    }
    Ok(out)
}

fn opt_str(
    props: &Map<String, Value>,
    name: &str,
    file: &str,
    index: usize,
) -> Result<Option<String>> {
    match props.get(name) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(bad(file, index, format!("`{name}` is not a string"))),
    }
}

fn resolve(
    file: &str,
    index: usize,
    props: Map<String, Value>,
    seen: &mut HashMap<String, u32>,
) -> Result<SuiteTest> {
    let src = opt_str(&props, "src", file, index)?.ok_or_else(|| {
        bad(
            file,
            index,
            "no `src` (neither on the test nor in the defaults)",
        )
    })?;
    let locale = opt_str(&props, "locale", file, index)?.ok_or_else(|| {
        bad(
            file,
            index,
            "no `locale` (neither on the test nor in the defaults)",
        )
    })?;
    let bidi_isolation = opt_str(&props, "bidiIsolation", file, index)?;
    if let Some(b) = &bidi_isolation
        && b != "default"
        && b != "none"
    {
        return Err(bad(file, index, format!("unknown `bidiIsolation` {b:?}")));
    }
    let params = match props.get("params") {
        None => None,
        Some(v @ Value::Array(_)) => Some(v.clone()),
        Some(_) => return Err(bad(file, index, "`params` is not an array")),
    };
    let tags = match props.get("tags") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|t| {
                t.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad(file, index, "a tag is not a string"))
            })
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(file, index, "`tags` is not an array")),
    };
    let exp = opt_str(&props, "exp", file, index)?;
    let exp_parts = match props.get("expParts") {
        None => None,
        Some(v @ Value::Array(_)) => Some(v.clone()),
        Some(_) => return Err(bad(file, index, "`expParts` is not an array")),
    };
    let exp_errors = match props.get("expErrors") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|e| {
                e.get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| bad(file, index, "an `expErrors` entry has no string `type`"))
            })
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(file, index, "`expErrors` is not an array")),
    };
    let description = opt_str(&props, "description", file, index)?;

    let hash = key_hash(&src, params.as_ref(), &locale, bidi_isolation.as_deref());
    let nth = seen.entry(hash.clone()).or_insert(0);
    let key = TestKey {
        file: file.to_owned(),
        hash,
        nth: *nth,
    };
    *nth += 1;
    let kind = TestKind::classify(exp_errors.iter().map(String::as_str));
    let resolved = canonical_json(&Value::Object(props));
    Ok(SuiteTest {
        key,
        index,
        kind,
        src,
        locale,
        params,
        bidi_isolation,
        tags,
        exp,
        exp_parts,
        exp_errors,
        description,
        resolved,
    })
}

/// What changed between two versions of the suite, by ledger key.
#[derive(Debug, Default)]
pub struct SuiteDiff<'a> {
    /// In `new` only.
    pub added: Vec<&'a SuiteTest>,
    /// In `old` only.
    pub removed: Vec<&'a SuiteTest>,
    /// Same key, different resolved properties (expectations, description, tags…): `(old, new)`.
    pub changed: Vec<(&'a SuiteTest, &'a SuiteTest)>,
    /// Same key and properties, different position in the file: `(old, new)`.
    pub moved: Vec<(&'a SuiteTest, &'a SuiteTest)>,
}

impl SuiteDiff<'_> {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.moved.is_empty()
    }
}

/// Compares two suites by ledger key.
pub fn diff<'a>(old: &'a Suite, new: &'a Suite) -> SuiteDiff<'a> {
    let old_by_key: HashMap<&TestKey, &SuiteTest> = old.tests.iter().map(|t| (&t.key, t)).collect();
    let new_by_key: HashMap<&TestKey, &SuiteTest> = new.tests.iter().map(|t| (&t.key, t)).collect();
    let mut d = SuiteDiff::default();
    for t in &new.tests {
        match old_by_key.get(&t.key) {
            None => d.added.push(t),
            Some(o) if o.resolved != t.resolved => d.changed.push((o, t)),
            Some(o) if o.index != t.index => d.moved.push((o, t)),
            Some(_) => {}
        }
    }
    for t in &old.tests {
        if !new_by_key.contains_key(&t.key) {
            d.removed.push(t);
        }
    }
    d
}
