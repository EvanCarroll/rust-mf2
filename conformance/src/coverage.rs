//! The spec coverage matrix (plans/01-conformance.md §5): every normative
//! statement of the vendored specification, and the tests that cover it.
//!
//! # What a normative statement is
//!
//! A sentence of `spec/**/*.md` that uses a BCP 14 key word (`MUST`, `MUST
//! NOT`, `REQUIRED`, `SHALL`, `SHALL NOT`, `SHOULD`, `SHOULD NOT`,
//! `RECOMMENDED`, `NOT RECOMMENDED`, `MAY`, `OPTIONAL`) in capitals, as
//! `intro.md` defines them. A key word written in double quotes is a mention,
//! not a use (the key-word paragraph itself). Fenced code is skipped. A block
//! is a paragraph, a list item, a table row or a blockquote paragraph; a block
//! is cut into sentences at `.`, `!` or `?` followed by a space and a capital,
//! `_`, `*`, `` ` `` or `"`, except after the abbreviations in [`ABBREVIATIONS`].
//!
//! # The id, byte for byte
//!
//! `<file>#<hash>`, where `file` is the path below `spec/` with `/`
//! separators and `hash` is the first 4 bytes of SHA-256, as 8 lowercase hex
//! digits, over the sentence's UTF-8 with every run of whitespace replaced by
//! one space and the ends trimmed (blockquote markers are removed first). A
//! sentence that occurs twice in one file gets `-1`, `-2`, … on its later
//! occurrences. An unchanged sentence keeps its id across a `spec-sync`; a
//! changed one gets a new id, so its coverage has to be looked at again.
//!
//! # The matrix
//!
//! `conformance/coverage.toml` has one `[[statement]]` per id: `says` (our
//! paraphrase — the matrix never quotes the spec, whose text may not be
//! redistributed), and either `tests` (at least one [`TestRef`]) or `na`
//! (`{ kind, reason }`, see [`NaKind`]). [`check`] holds it to the spec and
//! the tree; [`render`] writes `conformance/COVERAGE.md` from it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::suite::Suite;
use crate::{Error, Result};

/// The matrix, relative to the repository root.
pub const COVERAGE_TOML: &str = "conformance/coverage.toml";
/// The rendered matrix, relative to the repository root.
pub const COVERAGE_MD: &str = "conformance/COVERAGE.md";

/// The BCP 14 key words, longest first so that `MUST NOT` wins over `MUST`.
pub const KEYWORDS: [&str; 11] = [
    "NOT RECOMMENDED",
    "MUST NOT",
    "SHALL NOT",
    "SHOULD NOT",
    "RECOMMENDED",
    "REQUIRED",
    "OPTIONAL",
    "SHOULD",
    "SHALL",
    "MUST",
    "MAY",
];

/// Words ending in `.` that do not end a sentence.
pub const ABBREVIATIONS: [&str; 5] = ["e.g.", "i.e.", "etc.", "vs.", "cf."];

/// One normative statement of the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// `<file>#<hash>[-n]` (module documentation).
    pub id: String,
    /// Path below `spec/`, `/`-separated.
    pub file: String,
    /// The nearest heading above it.
    pub section: String,
    /// 1-based line of the block it starts in.
    pub line: usize,
    /// The key words it uses, strongest-first in [`KEYWORDS`] order, distinct.
    pub keywords: Vec<&'static str>,
    /// The sentence, whitespace collapsed (never written to the matrix).
    pub text: String,
}

impl Statement {
    /// Whether every key word it uses only grants a permission.
    pub fn is_permission(&self) -> bool {
        self.keywords
            .iter()
            .all(|k| matches!(*k, "MAY" | "OPTIONAL"))
    }
}

/// Every normative statement below `spec_dir`, in file then line order.
pub fn statements(spec_dir: &Path) -> Result<Vec<Statement>> {
    let mut files = Vec::new();
    collect_md(spec_dir, "", &mut files)?;
    files.sort();
    let mut out = Vec::new();
    for rel in files {
        let path = spec_dir.join(&rel);
        let text = fs::read_to_string(&path).map_err(|source| Error::IoAt {
            path: path.clone(),
            source,
        })?;
        out.extend(statements_in(&rel, &text));
    }
    Ok(out)
}

fn collect_md(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|source| Error::IoAt {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if entry.file_type()?.is_dir() {
            collect_md(&entry.path(), &child, out)?;
        } else if has_extension(&name, "md") {
            out.push(child);
        }
    }
    Ok(())
}

/// The statements of one file's text; `file` becomes their id prefix.
pub fn statements_in(file: &str, text: &str) -> Vec<Statement> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for block in blocks(text) {
        for sentence in sentences(&block.text) {
            let keywords = keywords_in(&sentence);
            if keywords.is_empty() {
                continue;
            }
            let hash = short_hash(&sentence);
            let n = seen.entry(hash.clone()).or_insert(0);
            let id = if *n == 0 {
                format!("{file}#{hash}")
            } else {
                format!("{file}#{hash}-{n}")
            };
            *n += 1;
            out.push(Statement {
                id,
                file: file.to_owned(),
                section: block.section.clone(),
                line: block.line,
                keywords,
                text: sentence,
            });
        }
    }
    out
}

struct Block {
    section: String,
    line: usize,
    text: String,
}

fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut fenced = false;
    let mut cur: Option<Block> = None;
    let flush = |cur: &mut Option<Block>, out: &mut Vec<Block>| {
        if let Some(b) = cur.take()
            && !b.text.trim().is_empty()
        {
            out.push(b);
        }
    };
    for (i, raw) in text.lines().enumerate() {
        let mut line = raw.trim_start();
        // Blockquote markers, however deep.
        while let Some(rest) = line.strip_prefix('>') {
            line = rest.trim_start();
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            flush(&mut cur, &mut out);
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if line.is_empty() || line.starts_with("[!") || line.starts_with("<!--") {
            flush(&mut cur, &mut out);
            continue;
        }
        if line.starts_with('#') {
            flush(&mut cur, &mut out);
            line.trim_start_matches('#').trim().clone_into(&mut section);
            continue;
        }
        let starts_item = line.starts_with("- ")
            || line.starts_with("* ")
            || line.starts_with("+ ")
            || line.starts_with('|')
            || numbered_item(line);
        if starts_item {
            flush(&mut cur, &mut out);
            line = strip_bullet(line);
        }
        match &mut cur {
            Some(b) => {
                b.text.push(' ');
                b.text.push_str(line);
            }
            None => {
                cur = Some(Block {
                    section: section.clone(),
                    line: i + 1,
                    text: line.to_owned(),
                });
            }
        }
        if line.starts_with('|') {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// A list item's text without its bullet or number (a table row is kept).
fn strip_bullet(line: &str) -> &str {
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return rest;
        }
    }
    if numbered_item(line) {
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        return &line[digits + 2..];
    }
    line
}

fn numbered_item(line: &str) -> bool {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && line[digits..].starts_with(". ")
}

/// `text` cut into sentences, each whitespace-collapsed.
pub fn sentences(text: &str) -> Vec<String> {
    let collapsed = collapse(text);
    let chars: Vec<(usize, char)> = collapsed.char_indices().collect();
    let mut out = Vec::new();
    let mut start = 0;
    for (k, &(at, c)) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let (Some(&(_, space)), Some(&(_, next))) = (chars.get(k + 1), chars.get(k + 2)) else {
            continue;
        };
        if space != ' ' || !(next.is_uppercase() || matches!(next, '_' | '*' | '`' | '"')) {
            continue;
        }
        let end = at + c.len_utf8();
        let head = &collapsed[start..end];
        let last_word = head.rsplit(' ').next().unwrap_or(head);
        if ABBREVIATIONS.contains(&last_word.to_ascii_lowercase().as_str()) {
            continue;
        }
        out.push(head.trim().to_owned());
        start = end;
    }
    let tail = collapsed[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_owned());
    }
    out
}

fn has_extension(path: &str, ext: &str) -> bool {
    Path::new(path).extension().is_some_and(|e| e == ext)
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The key words `sentence` uses (not mentions in double quotes).
pub fn keywords_in(sentence: &str) -> Vec<&'static str> {
    let bytes = sentence.as_bytes();
    let mut found = BTreeSet::new();
    let mut i = 0;
    while i < bytes.len() {
        let boundary_before = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let hit = boundary_before
            .then(|| {
                KEYWORDS.iter().position(|k| {
                    bytes[i..].starts_with(k.as_bytes())
                        && bytes
                            .get(i + k.len())
                            .is_none_or(|b| !b.is_ascii_alphanumeric())
                })
            })
            .flatten();
        match hit {
            Some(p) => {
                let k = KEYWORDS[p];
                let quoted = i > 0 && bytes[i - 1] == b'"' && bytes.get(i + k.len()) == Some(&b'"');
                if !quoted {
                    found.insert(p);
                }
                i += k.len();
            }
            None => i += 1,
        }
    }
    found.into_iter().map(|p| KEYWORDS[p]).collect()
}

fn short_hash(sentence: &str) -> String {
    let digest = Sha256::digest(sentence.as_bytes());
    let mut s = String::with_capacity(8);
    for b in &digest[..4] {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Why a statement needs no test of ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NaKind {
    /// A `MAY` / `OPTIONAL` we do not take up (only on a permission).
    Permission,
    /// Addressed to message authors, tool writers or future versions of the
    /// spec, not to an implementation's behaviour.
    NotImplementation,
    /// A property no test can observe from outside (e.g. "at most once" of a
    /// pure computation); `reason` says what makes it hold.
    ByConstruction,
}

impl NaKind {
    /// The name as written in the matrix.
    pub fn as_str(self) -> &'static str {
        match self {
            NaKind::Permission => "permission",
            NaKind::NotImplementation => "not-implementation",
            NaKind::ByConstruction => "by-construction",
        }
    }
}

/// `na = { kind, reason }`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Na {
    pub kind: NaKind,
    pub reason: String,
}

/// One `[[statement]]` of the matrix.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub says: String,
    #[serde(default)]
    pub tests: Vec<String>,
    pub na: Option<Na>,
    /// What we chose, where the statement leaves a choice.
    pub note: Option<String>,
}

/// `conformance/coverage.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    #[serde(default, rename = "statement")]
    pub entries: Vec<Entry>,
}

impl Coverage {
    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(COVERAGE_TOML);
        let text = fs::read_to_string(&path).map_err(|source| Error::IoAt {
            path: path.clone(),
            source,
        })?;
        Self::parse(&text)
    }
}

/// A test named by the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRef {
    /// `<suite file>@<hash>[/<nth>]`: a WG suite test, or one of ours under
    /// `extra/`, by its ledger key.
    Suite {
        file: String,
        hash: String,
        nth: u32,
    },
    /// `<path>::<name>`: in a `.rs` file, a `fn <name>(`; in any other file
    /// (a browser check), the text `<name>`. `path` is from the repository
    /// root.
    Code { path: String, name: String },
}

impl TestRef {
    pub fn parse(s: &str) -> Option<Self> {
        if let Some((path, name)) = s.split_once("::") {
            return (!path.is_empty() && !name.is_empty()).then(|| TestRef::Code {
                path: path.to_owned(),
                name: name.to_owned(),
            });
        }
        let (file, rest) = s.split_once('@')?;
        let (hash, nth) = match rest.split_once('/') {
            Some((h, n)) => (h, n.parse().ok()?),
            None => (rest, 0),
        };
        let hex = hash.len() == 8 && hash.bytes().all(|b| b.is_ascii_hexdigit());
        (has_extension(file, "json") && hex).then(|| TestRef::Suite {
            file: file.to_owned(),
            hash: hash.to_owned(),
            nth,
        })
    }
}

/// A way the matrix disagrees with the spec or the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gap {
    /// A statement of the spec with no entry: uncovered.
    Missing { id: String, line: usize },
    /// An entry for no statement of the spec (it changed, or was removed).
    Stale { id: String },
    /// Two entries for one id.
    Duplicate { id: String },
    /// An entry with neither `tests` nor `na`, or with both.
    Undecided { id: String },
    /// `na.kind = "permission"` on a statement that is not a permission.
    NotAPermission { id: String },
    /// An `na` or `says` left empty.
    Empty { id: String, field: &'static str },
    /// A test reference that does not parse or names nothing in the tree.
    Dangling { id: String, test: String },
}

impl fmt::Display for Gap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gap::Missing { id, line } => write!(f, "{id} (line {line}): no entry — uncovered"),
            Gap::Stale { id } => write!(f, "{id}: entry for no statement of the spec"),
            Gap::Duplicate { id } => write!(f, "{id}: more than one entry"),
            Gap::Undecided { id } => write!(f, "{id}: needs exactly one of `tests` and `na`"),
            Gap::NotAPermission { id } => {
                write!(
                    f,
                    "{id}: `permission` on a statement that requires something"
                )
            }
            Gap::Empty { id, field } => write!(f, "{id}: `{field}` is empty"),
            Gap::Dangling { id, test } => write!(f, "{id}: `{test}` names no test"),
        }
    }
}

/// Every [`Gap`] between the matrix, the spec's statements, the suite and
/// the tree at `root`. Empty means every normative statement is covered.
pub fn check(
    statements: &[Statement],
    coverage: &Coverage,
    suite: &Suite,
    root: &Path,
) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let by_id: BTreeMap<&str, &Statement> = statements.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut entries: BTreeMap<&str, &Entry> = BTreeMap::new();
    for e in &coverage.entries {
        if entries.insert(e.id.as_str(), e).is_some() {
            gaps.push(Gap::Duplicate { id: e.id.clone() });
        }
    }
    for s in statements {
        if !entries.contains_key(s.id.as_str()) {
            gaps.push(Gap::Missing {
                id: s.id.clone(),
                line: s.line,
            });
        }
    }
    let suite_keys: BTreeSet<(&str, &str, u32)> = suite
        .tests()
        .iter()
        .map(|t| (t.key.file.as_str(), t.key.hash.as_str(), t.key.nth))
        .collect();
    let mut files: BTreeMap<PathBuf, Option<String>> = BTreeMap::new();
    for e in &coverage.entries {
        let Some(s) = by_id.get(e.id.as_str()) else {
            gaps.push(Gap::Stale { id: e.id.clone() });
            continue;
        };
        if e.says.trim().is_empty() {
            gaps.push(Gap::Empty {
                id: e.id.clone(),
                field: "says",
            });
        }
        match (&e.na, e.tests.is_empty()) {
            (Some(_), false) | (None, true) => gaps.push(Gap::Undecided { id: e.id.clone() }),
            (Some(na), true) => {
                if na.kind == NaKind::Permission && !s.is_permission() {
                    gaps.push(Gap::NotAPermission { id: e.id.clone() });
                }
                if na.reason.trim().is_empty() {
                    gaps.push(Gap::Empty {
                        id: e.id.clone(),
                        field: "na.reason",
                    });
                }
            }
            (None, false) => {}
        }
        for t in &e.tests {
            let found = match TestRef::parse(t) {
                None => false,
                Some(TestRef::Suite { file, hash, nth }) => {
                    suite_keys.contains(&(file.as_str(), hash.as_str(), nth))
                }
                Some(TestRef::Code { path, name }) => {
                    let text = files
                        .entry(root.join(&path))
                        .or_insert_with_key(|p| fs::read_to_string(p).ok());
                    text.as_deref().is_some_and(|text| {
                        if has_extension(&path, "rs") {
                            text.contains(&format!("fn {name}("))
                        } else {
                            text.contains(name.as_str())
                        }
                    })
                }
            };
            if !found {
                gaps.push(Gap::Dangling {
                    id: e.id.clone(),
                    test: t.clone(),
                });
            }
        }
    }
    gaps
}

/// The spec pin as the rendered matrix names it — `` `<commit>` (<date>) ``
/// from `third_party/message-format-wg/PIN` — or `None` if it cannot be read.
pub fn pin(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("third_party/message-format-wg/PIN")).ok()?;
    let field = |name: &str| {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == name).then(|| v.trim().to_owned())
        })
    };
    Some(format!(
        "`unicode-org/message-format-wg` @ `{}` ({})",
        field("commit")?,
        field("date").unwrap_or_default()
    ))
}

/// `conformance/COVERAGE.md`: the matrix by spec file and section, with the
/// suite tests shown by file and index.
pub fn render(
    statements: &[Statement],
    coverage: &Coverage,
    suite: &Suite,
    pin: Option<&str>,
) -> String {
    let entries: BTreeMap<&str, &Entry> = coverage
        .entries
        .iter()
        .map(|e| (e.id.as_str(), e))
        .collect();
    let index: BTreeMap<(&str, &str, u32), usize> = {
        let mut per_file: BTreeMap<&str, usize> = BTreeMap::new();
        suite
            .tests()
            .iter()
            .map(|t| {
                let n = per_file.entry(t.key.file.as_str()).or_insert(0);
                let i = *n;
                *n += 1;
                ((t.key.file.as_str(), t.key.hash.as_str(), t.key.nth), i)
            })
            .collect()
    };
    let mut tested = 0;
    let mut na: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut missing = 0;
    for s in statements {
        match entries.get(s.id.as_str()) {
            Some(e) if !e.tests.is_empty() => tested += 1,
            Some(Entry { na: Some(n), .. }) => *na.entry(n.kind.as_str()).or_insert(0) += 1,
            _ => missing += 1,
        }
    }

    let mut out = String::new();
    let _ = writeln!(out, "# Spec coverage\n");
    let _ = writeln!(
        out,
        "Generated by `cargo xtask conformance-report` from `conformance/coverage.toml`; \
         do not edit. Every sentence of the vendored spec that uses a BCP 14 key word, \
         and the tests that cover it (plans/01-conformance.md §5; the extraction rule and \
         the id are in `conformance/src/coverage.rs`). The statements are paraphrased: \
         the spec text is not redistributed.\n"
    );
    if let Some(pin) = pin {
        let _ = writeln!(out, "Spec: {pin}\n");
    }
    let _ = writeln!(out, "| | Statements |\n|---|---:|");
    let _ = writeln!(out, "| covered by tests | {tested} |");
    for (kind, n) in &na {
        let _ = writeln!(out, "| n/a: {kind} | {n} |");
    }
    let _ = writeln!(out, "| **uncovered** | **{missing}** |");
    let _ = writeln!(out, "| total | {} |", statements.len());
    out.push_str(
        "\nTests are named `file #index` (a suite test; `extra/` is ours, in the WG \
         schema) or `path::name` (a Rust test or a browser check).\n",
    );

    let mut file = "";
    let mut section = "";
    for s in statements {
        if s.file != file {
            file = &s.file;
            section = "";
            let _ = writeln!(out, "\n## `{file}`");
        }
        if s.section != section {
            section = &s.section;
            let _ = writeln!(
                out,
                "\n### {section}\n\n| Line | Key words | Statement | Covered by |\n|---:|---|---|---|"
            );
        }
        let (says, by) = match entries.get(s.id.as_str()) {
            None => ("—".to_owned(), "**uncovered**".to_owned()),
            Some(e) => {
                let mut says = cell(&e.says);
                if let Some(n) = &e.note {
                    let _ = write!(says, " *{}*", cell(n));
                }
                let by = match &e.na {
                    Some(na) => format!("n/a ({}): {}", na.kind.as_str(), cell(&na.reason)),
                    None => e
                        .tests
                        .iter()
                        .map(|t| match TestRef::parse(t) {
                            Some(TestRef::Suite { file, hash, nth }) => {
                                match index.get(&(file.as_str(), hash.as_str(), nth)) {
                                    Some(i) => format!("`{file}` #{i}"),
                                    None => format!("`{t}`"),
                                }
                            }
                            _ => format!("`{t}`"),
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                (says, by)
            }
        };
        let _ = writeln!(
            out,
            "| {} | {} | {says} | {by} |",
            s.line,
            s.keywords.join(", ")
        );
    }
    out
}

fn cell(s: &str) -> String {
    collapse(s).replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_split_at_a_stop_before_a_capital_but_not_after_an_abbreviation() {
        assert_eq!(
            sentences("A _x_ MUST be.  It MAY,\n e.g. Here, be `y`. _Z_ SHOULD."),
            [
                "A _x_ MUST be.",
                "It MAY, e.g. Here, be `y`.",
                "_Z_ SHOULD."
            ]
        );
    }

    #[test]
    fn quoted_key_words_are_mentions() {
        assert_eq!(
            keywords_in(r#"The key words "MUST" and "MAY" are"#),
            Vec::<&str>::new()
        );
        assert_eq!(keywords_in("It MUST NOT, and MAY"), ["MUST NOT", "MAY"]);
        assert_eq!(
            keywords_in("MUSTARD and NOT RECOMMENDED"),
            ["NOT RECOMMENDED"]
        );
    }

    #[test]
    fn code_is_skipped_and_duplicates_are_numbered() {
        let text = "# S\n\nIt MUST.\n\n```\nIt MUST.\n```\n\n> It MUST.\n\n- It MAY.\n";
        let s = statements_in("f.md", text);
        let ids: Vec<&str> = s.iter().map(|s| s.id.as_str()).collect();
        let h = short_hash("It MUST.");
        assert_eq!(
            ids,
            [
                format!("f.md#{h}"),
                format!("f.md#{h}-1"),
                format!("f.md#{}", short_hash("It MAY."))
            ]
        );
        assert_eq!((s[0].line, s[1].line, s[2].line), (3, 9, 11));
        assert!(s.iter().all(|s| s.section == "S"));
    }

    #[test]
    fn test_refs_parse() {
        assert_eq!(
            TestRef::parse("functions/number.json@0a1b2c3d/1"),
            Some(TestRef::Suite {
                file: "functions/number.json".into(),
                hash: "0a1b2c3d".into(),
                nth: 1
            })
        );
        assert_eq!(
            TestRef::parse("crates/x/tests/y.rs::z"),
            Some(TestRef::Code {
                path: "crates/x/tests/y.rs".into(),
                name: "z".into()
            })
        );
        assert_eq!(TestRef::parse("syntax.json@xyz"), None);
    }
}
