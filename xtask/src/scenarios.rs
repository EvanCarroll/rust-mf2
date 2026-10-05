//! `cargo xtask scenarios`: the incremental-rebuild scenarios of P0.9, on the
//! real pipeline (Phase 5a, A9).
//!
//! P0.9 asked what cargo rebuilds when a locale changes, with a stand-in for
//! `mf2-build`. These ask the same of the pipeline itself, over
//! `tools/i18n-fixture`: after each edit, what did the build rewrite, did the
//! manifest hash move, and is the client `.wasm` still the same file?
//!
//! The one that matters most is the translation-only edit: a translator
//! changing a string must not change the wasm, or every visitor would
//! re-download the application because someone fixed a typo.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cmd;
use crate::error::{Error, Result};

const FIXTURE: &str = "mf2-i18n-fixture";
const CLIENT_BIN: &str = "mf2-i18n-client";
const WASM: &str = "wasm32-unknown-unknown";

/// What one step of the run observed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    /// Every file in the build script's output directory, by name and hash.
    outputs: BTreeMap<String, String>,
    /// `MANIFEST_HASH` as the generated module states it.
    manifest_hash: String,
    /// The client artifact's hash.
    wasm: String,
}

impl Snapshot {
    /// The output files that differ from `other`'s, and those only here.
    fn changed(&self, other: &Snapshot) -> Vec<String> {
        let mut out: Vec<String> = self
            .outputs
            .iter()
            .filter(|(name, hash)| other.outputs.get(*name) != Some(hash))
            .map(|(name, _)| name.clone())
            .collect();
        out.extend(
            other
                .outputs
                .keys()
                .filter(|name| !self.outputs.contains_key(*name))
                .cloned(),
        );
        out.sort();
        out.dedup();
        out
    }
}

/// One scenario: an edit, and what it is allowed to change.
struct Scenario {
    name: &'static str,
    what: &'static str,
    /// Applies the edit; `None` changes nothing.
    edit: fn(&Path) -> Result<()>,
    /// Whether the manifest hash is expected to move.
    manifest_moves: bool,
    /// Whether the client artifact is expected to differ.
    wasm_moves: bool,
    /// Which of the i18n crate's outputs must change, by suffix; empty means
    /// "none".
    changes: &'static [&'static str],
    /// The same with the catalogs emitted apart: the i18n crate then writes
    /// no catalog at all, so only what the manifest covers can move.
    split_changes: &'static [&'static str],
}

fn locale(root: &Path, tag: &str) -> PathBuf {
    root.join("locales").join(tag).join("main.mf2")
}

fn edit_file(path: &Path, from: &str, to: &str) -> Result<()> {
    let text = crate::fsx::read_to_string(path)?;
    if !text.contains(from) {
        return Err(Error::CommandFailed {
            command: format!("edit {}", path.display()),
            status: "no match".to_owned(),
            stderr: format!("{from:?} is not in the file"),
        });
    }
    crate::fsx::write(path, text.replacen(from, to, 1).as_bytes())
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "S1 nothing changed",
            what: "a second build with no edit at all",
            edit: |_| Ok(()),
            manifest_moves: false,
            wasm_moves: false,
            changes: &[],
            split_changes: &[],
        },
        Scenario {
            name: "S2 mtime only",
            what: "a locale file touched but not changed",
            edit: |root| {
                let path = locale(root, "pl");
                let text = crate::fsx::read_to_string(&path)?;
                crate::fsx::write(&path, text.as_bytes())
            },
            manifest_moves: false,
            wasm_moves: false,
            changes: &[],
            split_changes: &[],
        },
        Scenario {
            name: "S3 translation only",
            what: "one message's text changed in a translation",
            edit: |root| edit_file(&locale(root, "pl"), "plain = Zapisz", "plain = Zachowaj"),
            manifest_moves: false,
            wasm_moves: false,
            changes: &["pl"],
            split_changes: &[],
        },
        Scenario {
            name: "S4 source text",
            what: "one message's text changed in the source locale",
            edit: |root| edit_file(&locale(root, "en"), "plain = Save", "plain = Store"),
            manifest_moves: false,
            wasm_moves: false,
            // Only the source locale's: `ar` has this message of its own, so
            // it does not take the new text.
            changes: &["en"],
            split_changes: &[],
        },
        Scenario {
            name: "S5 new id",
            what: "a message added to the source locale",
            edit: |root| {
                let path = locale(root, "en");
                let text = crate::fsx::read_to_string(&path)?;
                crate::fsx::write(&path, format!("{text}\nadded = Just added\n").as_bytes())
            },
            manifest_moves: true,
            wasm_moves: true,
            changes: &["en", "ar", "pl", "manifest.mf2m", "mf2_generated.rs"],
            split_changes: &["manifest.mf2m", "mf2_generated.rs"],
        },
        Scenario {
            name: "S6 new input",
            what: "a variable added to a source message",
            edit: |root| {
                edit_file(
                    &locale(root, "en"),
                    "greeting = Hello, {$name}!",
                    "greeting = Hello, {$name} ({$role})!",
                )
            },
            manifest_moves: true,
            wasm_moves: true,
            changes: &["en", "ar", "manifest.mf2m", "mf2_generated.rs"],
            split_changes: &["manifest.mf2m", "mf2_generated.rs"],
        },
    ]
}

pub(crate) fn run(root: &Path, keep: bool, split: bool) -> Result<()> {
    let cargo = cmd::cargo();
    let fixture = root.join("tools").join("i18n-fixture");
    let saved = save(&fixture)?;
    let features = if split {
        "hydrate,host-web-number-plain,split-catalogs"
    } else {
        "hydrate,host-web-number-plain"
    };
    eprintln!(
        "scenarios: {} arrangement ({features})",
        if split {
            "the catalogs apart"
        } else {
            "one crate"
        }
    );
    let result = go(&cargo, root, &fixture, features);
    if !keep {
        restore(&saved)?;
        // Leave the tree as it was found, artifacts included.
        build(&cargo, root, features)?;
    }
    result
}

fn go(cargo: &OsStr, root: &Path, fixture: &Path, features: &str) -> Result<()> {
    let split = features.contains("split-catalogs");
    let mut report = String::new();
    let _ = writeln!(
        report,
        "| scenario | outputs rewritten | manifest hash | client wasm | rebuild |"
    );
    let _ = writeln!(report, "|---|---|---|---|---:|");

    let out = build(cargo, root, features)?;
    let mut previous = snapshot(root, &out)?;
    eprintln!(
        "scenarios: baseline — manifest {}, wasm {}",
        previous.manifest_hash, previous.wasm
    );

    let mut problems: Vec<String> = Vec::new();
    for scenario in scenarios() {
        (scenario.edit)(fixture)?;
        let started = Instant::now();
        let out = build(cargo, root, features)?;
        let rebuild = started.elapsed();
        let now = snapshot(root, &out)?;
        let changed = now.changed(&previous);

        let manifest_moved = now.manifest_hash != previous.manifest_hash;
        let wasm_moved = now.wasm != previous.wasm;
        if manifest_moved != scenario.manifest_moves {
            problems.push(format!(
                "{}: the manifest hash {} ({} → {})",
                scenario.name,
                if manifest_moved {
                    "moved"
                } else {
                    "did not move"
                },
                previous.manifest_hash,
                now.manifest_hash
            ));
        }
        if wasm_moved != scenario.wasm_moves {
            problems.push(format!(
                "{}: the client wasm {}",
                scenario.name,
                if wasm_moved {
                    "changed and should not have"
                } else {
                    "did not change and should have"
                }
            ));
        }
        let expected_changes = if split {
            scenario.split_changes
        } else {
            scenario.changes
        };
        for expected in expected_changes {
            if !changed.iter().any(|name| name.contains(expected)) {
                problems.push(format!(
                    "{}: nothing matching {expected:?} was rewritten (got {changed:?})",
                    scenario.name
                ));
            }
        }
        if expected_changes.is_empty() && !changed.is_empty() {
            problems.push(format!(
                "{}: rewrote {changed:?} though nothing should have changed",
                scenario.name
            ));
        }
        let _ = writeln!(
            report,
            "| {} — {} | {} | {} | {} | {} |",
            scenario.name,
            scenario.what,
            if changed.is_empty() {
                "none".to_owned()
            } else {
                changed.join(", ")
            },
            if manifest_moved { "moved" } else { "same" },
            if wasm_moved {
                "rebuilt"
            } else {
                "**identical**"
            },
            format_duration(rebuild)
        );
        eprintln!(
            "scenarios: {:<22} outputs {:<40} manifest {} wasm {} rebuild {}",
            scenario.name,
            if changed.is_empty() {
                "none".to_owned()
            } else {
                changed.join(", ")
            },
            if manifest_moved { "moved" } else { "same " },
            if wasm_moved { "rebuilt" } else { "identical" },
            format_duration(rebuild)
        );
        previous = now;
    }

    print!("\n{report}");
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            command: "cargo xtask scenarios".to_owned(),
            status: format!("{} scenario(s) behaved unexpectedly", problems.len()),
            stderr: problems.join("\n"),
        })
    }
}

/// A duration for a table a person reads.
fn format_duration(d: Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}

/// Builds the client artifact, which runs the fixture's build script, and
/// gives the `OUT_DIR` cargo used for it.
///
/// The directory is read from cargo's own JSON rather than guessed from the
/// newest file under `target/`: one feature set has one build directory, and
/// a run that compares two arrangements would otherwise look at the other
/// one's outputs.
fn build(cargo: &OsStr, root: &Path, features: &str) -> Result<PathBuf> {
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("--message-format"),
        OsStr::new("json-render-diagnostics"),
        OsStr::new("-p"),
        OsStr::new(FIXTURE),
        OsStr::new("--bin"),
        OsStr::new(CLIENT_BIN),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
        OsStr::new("--target"),
        OsStr::new(WASM),
        OsStr::new("--release"),
    ];
    let stdout = cmd::run_capture(cargo, &args, root, &[])?;
    let text = String::from_utf8_lossy(&stdout);
    let mut out_dir = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value["reason"] == "build-script-executed"
            && value["package_id"]
                .as_str()
                .is_some_and(|id| id.contains(FIXTURE))
            && let Some(dir) = value["out_dir"].as_str()
        {
            out_dir = Some(PathBuf::from(dir));
        }
    }
    out_dir.ok_or_else(|| Error::CommandFailed {
        command: format!("cargo build -p {FIXTURE} --features {features}"),
        status: "no build script output".to_owned(),
        stderr: "cargo did not report an out_dir for the fixture".to_owned(),
    })
}

/// What the build wrote, and what the client carries.
fn snapshot(root: &Path, out: &Path) -> Result<Snapshot> {
    let out = out.to_path_buf();
    let mut outputs = BTreeMap::new();
    let entries = std::fs::read_dir(&out).map_err(|source| Error::IoAt {
        path: out.clone(),
        source,
    })?;
    let mut manifest_hash = String::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "mf2_generated.rs" {
            let text = String::from_utf8_lossy(&bytes);
            manifest_hash = text
                .lines()
                .find_map(|l| l.strip_prefix("pub const MANIFEST_HASH: u64 = "))
                .map(|l| l.trim_end_matches(';').to_owned())
                .unwrap_or_default();
        }
        outputs.insert(name, digest(&bytes));
    }
    let wasm = root
        .join("target")
        .join(WASM)
        .join("release")
        .join(format!("{CLIENT_BIN}.wasm"));
    let bytes = std::fs::read(&wasm).map_err(|source| Error::IoAt {
        path: wasm.clone(),
        source,
    })?;
    Ok(Snapshot {
        outputs,
        manifest_hash,
        wasm: digest(&bytes),
    })
}

/// A short content hash, for a report a person reads. FNV-1a 64: two files
/// that differ have to differ here, which is all a scenario asks of it.
fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The fixture's locale files, so that the run leaves the tree as it was.
fn save(fixture: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for tag in ["en", "pl", "ar"] {
        let path = locale(fixture, tag);
        out.push((path.clone(), crate::fsx::read_to_string(&path)?));
    }
    Ok(out)
}

fn restore(saved: &[(PathBuf, String)]) -> Result<()> {
    for (path, text) in saved {
        crate::fsx::write(path, text.as_bytes())?;
    }
    Ok(())
}
