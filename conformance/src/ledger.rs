//! `conformance/ledger.toml`: one entry per suite test, one status per column
//! (plans/01-conformance.md §4). This module parses and renders it; the rules
//! that relate it to the suite are in [`crate::check`](mod@crate::check).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use toml::{Table, Value};

use crate::error::{Error, Result};
use crate::key::TestKey;
use crate::matrix::{Column, Phase};
use crate::suite::Suite;

/// A ledger cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// The layer passes the test.
    Pass { via: Option<Via> },
    /// Known failure, fixed by `until`.
    Xfail {
        until: Phase,
        reason: Option<String>,
        via: Option<Via>,
    },
    /// Default-features columns only: the documented degradation.
    Degraded {
        kind: DegradedKind,
        detail: Option<String>,
    },
    /// The reason is a fact about the *test* (never a tag).
    Skip { reason: String },
    /// The layer cannot apply (the `n/a` matrix).
    NotApplicable,
}

impl Cell {
    /// The status word as written in the ledger.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Pass { .. } => "pass",
            Self::Xfail { .. } => "xfail",
            Self::Degraded { .. } => "degraded",
            Self::Skip { .. } => "skip",
            Self::NotApplicable => "n/a",
        }
    }

    fn via(&self) -> Option<Via> {
        match self {
            Self::Pass { via } | Self::Xfail { via, .. } => *via,
            _ => None,
        }
    }
}

/// How an L5 test is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Through the dynamic named-args API (the test's `params` deliberately
    /// mismatch the message).
    Dyn,
}

/// What a default-features configuration does instead of passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradedKind {
    UnsupportedOperation,
    UnknownFunction,
    BuildReject,
}

impl DegradedKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedOperation => "unsupported-operation",
            Self::UnknownFunction => "unknown-function",
            Self::BuildReject => "build-reject",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [
            Self::UnsupportedOperation,
            Self::UnknownFunction,
            Self::BuildReject,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

/// One `[[test]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: TestKey,
    /// Position in the file when the entry was written (a hint).
    pub index: usize,
    /// Missing columns are kept missing here and reported by the checker.
    pub cells: BTreeMap<Column, Cell>,
}

/// One `[[note]]`: a fact recorded once rather than per test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub status: String,
    pub reason: String,
}

/// The id of the note every ledger MUST carry (plans/01-conformance.md §2).
pub const SURROGATES_NOTE_ID: &str = "unpaired-surrogates";

/// The whole ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ledger {
    pub current_phase: Phase,
    pub notes: Vec<Note>,
    pub entries: Vec<Entry>,
}

fn err(context: impl Into<String>, message: impl Into<String>) -> Error {
    Error::Ledger {
        context: context.into(),
        message: message.into(),
    }
}

fn get_str<'a>(t: &'a Table, name: &str, ctx: &str) -> Result<&'a str> {
    match t.get(name) {
        Some(Value::String(s)) => Ok(s),
        Some(_) => Err(err(ctx, format!("`{name}` must be a string"))),
        None => Err(err(ctx, format!("`{name}` is missing"))),
    }
}

fn get_uint(t: &Table, name: &str, ctx: &str) -> Result<u64> {
    match t.get(name) {
        Some(Value::Integer(i)) => {
            u64::try_from(*i).map_err(|_| err(ctx, format!("`{name}` must not be negative")))
        }
        Some(_) => Err(err(ctx, format!("`{name}` must be an integer"))),
        None => Err(err(ctx, format!("`{name}` is missing"))),
    }
}

impl Ledger {
    /// Parses ledger TOML. Structural problems are errors; relations to the
    /// suite are checked separately ([`crate::check`](mod@crate::check)).
    pub fn parse(text: &str) -> Result<Self> {
        let root: Table = toml::from_str(text)?;
        for k in root.keys() {
            if !matches!(k.as_str(), "current_phase" | "note" | "test") {
                return Err(err("top level", format!("unknown key `{k}`")));
            }
        }
        let phase = get_str(&root, "current_phase", "top level")?;
        let current_phase = Phase::parse(phase)
            .ok_or_else(|| err("current_phase", format!("unknown phase {phase:?}")))?;

        let mut notes = Vec::new();
        for (i, v) in array_of_tables(&root, "note")?.into_iter().enumerate() {
            let ctx = format!("[[note]] #{i}");
            for k in v.keys() {
                if !matches!(k.as_str(), "id" | "status" | "reason") {
                    return Err(err(&ctx, format!("unknown key `{k}`")));
                }
            }
            notes.push(Note {
                id: get_str(v, "id", &ctx)?.to_owned(),
                status: get_str(v, "status", &ctx)?.to_owned(),
                reason: get_str(v, "reason", &ctx)?.to_owned(),
            });
        }

        let mut entries = Vec::new();
        for (i, v) in array_of_tables(&root, "test")?.into_iter().enumerate() {
            entries.push(parse_entry(i, v)?);
        }
        Ok(Self {
            current_phase,
            notes,
            entries,
        })
    }

    /// Renders the ledger in its canonical layout.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str(HEADER);
        let _ = writeln!(
            out,
            "current_phase = {}",
            toml_str(self.current_phase.as_str())
        );
        for n in &self.notes {
            out.push_str("\n[[note]]\n");
            let _ = writeln!(out, "id     = {}", toml_str(&n.id));
            let _ = writeln!(out, "status = {}", toml_str(&n.status));
            let _ = writeln!(out, "reason = {}", toml_str(&n.reason));
        }
        for e in &self.entries {
            out.push_str("\n[[test]]\n");
            let _ = writeln!(out, "file  = {}", toml_str(&e.key.file));
            let _ = writeln!(out, "index = {}", e.index);
            let _ = writeln!(out, "hash  = {}", toml_str(&e.key.hash));
            let _ = writeln!(out, "nth   = {}", e.key.nth);
            for (col, cell) in &e.cells {
                let _ = writeln!(out, "{col} = {}", render_cell(cell));
            }
        }
        out
    }

    /// The ledger `--init` generates: every applicable cell `xfail` until the
    /// phase of the layer → phase table, every other cell `n/a`.
    pub fn init(suite: &Suite) -> Self {
        let entries = suite
            .tests()
            .iter()
            .map(|t| Entry {
                key: t.key.clone(),
                index: t.index,
                cells: Column::ALL
                    .into_iter()
                    .map(|c| {
                        let cell = if t.kind.applies(c) {
                            Cell::Xfail {
                                until: c.deadline(&t.key.file),
                                reason: None,
                                via: None,
                            }
                        } else {
                            Cell::NotApplicable
                        };
                        (c, cell)
                    })
                    .collect(),
            })
            .collect();
        Self {
            current_phase: Phase::P0,
            notes: vec![Note {
                id: SURROGATES_NOTE_ID.to_owned(),
                status: "n/a".to_owned(),
                reason: SURROGATES_REASON.to_owned(),
            }],
            entries,
        }
    }
}

const HEADER: &str = "\
# The conformance ledger: one [[test]] per test of the vendored WG suite, one status per
# column (plans/01-conformance.md §3-§4). Checked by `cargo xtask conformance-report`,
# which writes conformance/REPORT.md; first generated by `--init`.
#
# Key = (file, hash, nth); the hash encoding is documented in conformance/src/key.rs.
# `index` is the test's 0-based position in its file: a hint, not part of the key.
# Columns: L1..L6 (all features on), L4d/L5d/L6d (default features).
# Statuses: \"pass\" | \"n/a\" | { status = \"xfail\", until = \"<phase>\", reason? }
#           | { status = \"degraded\", kind, detail? } (L4d/L5d/L6d) | { status = \"skip\", reason }
# L5/L5d cells may add via = \"dyn\". Tags are never a skip reason.

";

const SURROGATES_REASON: &str = "test/README.md asks implementations whose strings can hold \
unpaired surrogate code points to add syntax-error tests for them (e.g. src \"{\\ud800}\"). \
Neither the JSON suite nor a Rust &str can represent an unpaired surrogate, so such input \
cannot reach any layer: N/A by construction.";

fn array_of_tables<'a>(root: &'a Table, name: &str) -> Result<Vec<&'a Table>> {
    match root.get(name) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_table()
                    .ok_or_else(|| err(format!("[[{name}]]"), "must be an array of tables"))
            })
            .collect(),
        Some(_) => Err(err(format!("[[{name}]]"), "must be an array of tables")),
    }
}

fn parse_entry(i: usize, t: &Table) -> Result<Entry> {
    let ctx = format!("[[test]] #{i}");
    let file = get_str(t, "file", &ctx)?.to_owned();
    let hash = get_str(t, "hash", &ctx)?.to_owned();
    if hash.len() != 8 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(err(
            &ctx,
            format!("`hash` must be 8 lowercase hex digits, found {hash:?}"),
        ));
    }
    let nth =
        u32::try_from(get_uint(t, "nth", &ctx)?).map_err(|_| err(&ctx, "`nth` is too large"))?;
    let index = usize::try_from(get_uint(t, "index", &ctx)?)
        .map_err(|_| err(&ctx, "`index` is too large"))?;
    let key = TestKey { file, hash, nth };
    let ctx = format!("{ctx} ({key})");
    let mut cells = BTreeMap::new();
    for (k, v) in t {
        if matches!(k.as_str(), "file" | "hash" | "nth" | "index") {
            continue;
        }
        let col = Column::parse(k).ok_or_else(|| err(&ctx, format!("unknown column `{k}`")))?;
        let cell = parse_cell(v).map_err(|m| err(format!("{ctx} {col}"), m))?;
        cells.insert(col, cell);
    }
    Ok(Entry { key, index, cells })
}

fn parse_cell(v: &Value) -> std::result::Result<Cell, String> {
    let t = match v {
        Value::String(s) => {
            return match s.as_str() {
                "pass" => Ok(Cell::Pass { via: None }),
                "n/a" => Ok(Cell::NotApplicable),
                "xfail" => Err("`xfail` needs a table with `until`".to_owned()),
                "degraded" => Err("`degraded` needs a table with `kind`".to_owned()),
                "skip" => Err("`skip` needs a table with `reason`".to_owned()),
                other => Err(format!("unknown status {other:?}")),
            };
        }
        Value::Table(t) => t,
        _ => return Err("a cell must be a status string or an inline table".to_owned()),
    };
    let text = |name: &str| -> std::result::Result<Option<String>, String> {
        match t.get(name) {
            None => Ok(None),
            Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.clone())),
            Some(Value::String(_)) => Err(format!("`{name}` must not be empty")),
            Some(_) => Err(format!("`{name}` must be a string")),
        }
    };
    let status = text("status")?.ok_or("`status` is missing")?;
    let allowed: &[&str] = match status.as_str() {
        "pass" => &["status", "via"],
        "xfail" => &["status", "until", "reason", "via"],
        "degraded" => &["status", "kind", "detail"],
        "skip" => &["status", "reason"],
        "n/a" => &["status"],
        other => return Err(format!("unknown status {other:?}")),
    };
    if let Some(k) = t.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!("`{k}` is not allowed on a `{status}` cell"));
    }
    let via = match text("via")?.as_deref() {
        None => None,
        Some("dyn") => Some(Via::Dyn),
        Some(other) => return Err(format!("unknown `via` {other:?} (only \"dyn\")")),
    };
    Ok(match status.as_str() {
        "pass" => Cell::Pass { via },
        "xfail" => {
            let until = text("until")?.ok_or("`xfail` needs `until`")?;
            let until = Phase::parse(&until).ok_or_else(|| format!("unknown phase {until:?}"))?;
            Cell::Xfail {
                until,
                reason: text("reason")?,
                via,
            }
        }
        "degraded" => {
            let kind = text("kind")?.ok_or("`degraded` needs `kind`")?;
            let kind = DegradedKind::parse(&kind).ok_or_else(|| {
                format!("unknown degradation {kind:?} (unsupported-operation | unknown-function | build-reject)")
            })?;
            Cell::Degraded {
                kind,
                detail: text("detail")?,
            }
        }
        "skip" => Cell::Skip {
            reason: text("reason")?.ok_or("`skip` needs a `reason`")?,
        },
        _ => Cell::NotApplicable,
    })
}

fn render_cell(cell: &Cell) -> String {
    let mut fields: Vec<(&str, String)> = vec![("status", cell.status().to_owned())];
    match cell {
        Cell::Pass { via: None } => return toml_str("pass"),
        Cell::NotApplicable => return toml_str("n/a"),
        Cell::Pass { .. } => {}
        Cell::Xfail { until, reason, .. } => {
            if let Some(r) = reason {
                fields.push(("reason", r.clone()));
            }
            fields.push(("until", until.as_str().to_owned()));
        }
        Cell::Degraded { kind, detail } => {
            fields.push(("kind", kind.as_str().to_owned()));
            if let Some(d) = detail {
                fields.push(("detail", d.clone()));
            }
        }
        Cell::Skip { reason } => fields.push(("reason", reason.clone())),
    }
    if cell.via() == Some(Via::Dyn) {
        fields.push(("via", "dyn".to_owned()));
    }
    let inner: Vec<String> = fields
        .into_iter()
        .map(|(k, v)| format!("{k} = {}", toml_str(&v)))
        .collect();
    format!("{{ {} }}", inner.join(", "))
}

/// A TOML basic string.
fn toml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{Cell, DegradedKind, Ledger, Via, parse_cell, render_cell};
    use crate::matrix::Phase;

    #[test]
    fn cells_round_trip() {
        let cells = [
            Cell::Pass { via: None },
            Cell::Pass {
                via: Some(Via::Dyn),
            },
            Cell::NotApplicable,
            Cell::Xfail {
                until: Phase::P3,
                reason: Some("roundingIncrement \"not\" implemented".to_owned()),
                via: None,
            },
            Cell::Xfail {
                until: Phase::P5b,
                reason: None,
                via: Some(Via::Dyn),
            },
            Cell::Degraded {
                kind: DegradedKind::UnsupportedOperation,
                detail: Some("numberingSystem needs fn-number".to_owned()),
            },
            Cell::Skip {
                reason: "a fact about the test".to_owned(),
            },
        ];
        for cell in cells {
            let text = format!("c = {}", render_cell(&cell));
            let table: toml::Table = toml::from_str(&text).unwrap();
            assert_eq!(parse_cell(&table["c"]).unwrap(), cell, "{text}");
        }
    }

    #[test]
    fn plan_example_parses() {
        // The example entry of plans/01-conformance.md §4.
        let text = r#"
current_phase = "P0"

[[test]]
file  = "functions/number.json"
index = 17                  # position in the file at the pin (a hint, not the key)
hash  = "9c1e44aa"          # sha256(src ‖ params ‖ locale ‖ bidiIsolation)[..8]
nth   = 0                   # occurrence ordinal among tests with this hash in this file
L1 = "pass"
L2 = "pass"
L3 = "pass"
L4 = { status = "xfail", reason = "roundingIncrement not implemented", until = "P3" }
L5 = { status = "xfail", until = "P5b" }
L6 = { status = "xfail", until = "P6" }
L4d = { status = "degraded", kind = "unsupported-operation", detail = "numberingSystem needs fn-number" }
L5d = { status = "xfail", until = "P5b" }
L6d = { status = "xfail", until = "P6" }
"#;
        let ledger = Ledger::parse(text).unwrap();
        assert_eq!(ledger.entries.len(), 1);
        assert_eq!(ledger.entries[0].cells.len(), 9);
        let again = Ledger::parse(&ledger.to_toml()).unwrap();
        assert_eq!(again, ledger);
    }

    #[test]
    fn malformed_cells_are_rejected() {
        for bad in [
            r#"c = "xfail""#,
            r#"c = "passed""#,
            r#"c = { status = "xfail" }"#,
            r#"c = { status = "xfail", until = "P5" }"#,
            r#"c = { status = "pass", until = "P3" }"#,
            r#"c = { status = "skip" }"#,
            r#"c = { status = "degraded", kind = "slow" }"#,
            r#"c = { status = "pass", via = "static" }"#,
            r"c = 3",
        ] {
            let table: toml::Table = toml::from_str(bad).unwrap();
            assert!(parse_cell(&table["c"]).is_err(), "{bad}");
        }
    }
}
