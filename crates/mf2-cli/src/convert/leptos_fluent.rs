//! `mf2 convert --from leptos-fluent APP_DIR` (`plans/05-tooling.md` §6.2):
//! §6.1 on the application's `.ftl` files, then its Rust call sites.
//!
//! Nothing is written without `--write`: the command shows what it would
//! change — a unified diff per Rust file, the `.mf2` files it would write —
//! and the report of what is left to do by hand.

pub(crate) mod call;
pub(crate) mod rewrite;

// It generates the reference workload with `workload-gen`, which is not
// published: it runs in the workspace only (`build.rs`).
#[cfg(all(test, mf2_workspace))]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mf2_build::Config;
use mf2_build::Layout;

use self::rewrite::{Messages, Options};
use super::report::{Code, Finding, Report};
use super::{convert_fluent, fluent};
use crate::Format;
use crate::error::{Error, Result, read, write};

/// What the command was asked.
pub(crate) struct Request<'a> {
    /// The application's crate.
    pub(crate) app: &'a Path,
    pub(crate) write: bool,
    pub(crate) locales: Option<&'a Path>,
    pub(crate) i18n_crate: Option<&'a str>,
    pub(crate) format: Format,
}

/// One changed Rust file: its path, what it was, what it becomes.
pub(crate) struct Change {
    pub(crate) path: PathBuf,
    pub(crate) before: String,
    pub(crate) after: String,
}

pub(crate) fn run(dir: &Path, request: &Request<'_>) -> Result<()> {
    let app = request.app;
    if !app.join("Cargo.toml").is_file() {
        return Err(Error::Usage(format!(
            "{}: no Cargo.toml; APP_DIR is the application's crate",
            app.display()
        )));
    }
    let skip = skipped_dir(app, dir);
    let mut rust = Vec::new();
    let mut manifests = Vec::new();
    collect(app, skip.as_deref(), &mut rust, &mut manifests)?;
    rust.sort();
    manifests.sort();

    let mut sources = Vec::with_capacity(rust.len());
    for path in &rust {
        sources.push((path.clone(), read(path)?));
    }

    // The messages first, so that every call can be checked against them.
    let locales = match request.locales {
        Some(dir) => dir.to_path_buf(),
        None => sources
            .iter()
            .find_map(|(_, text)| rewrite::initializer_locales(text))
            .map_or_else(|| app.join("locales"), |rel| app.join(rel)),
    };
    let mut report = Report::default();
    let outputs = convert_fluent(
        &locales,
        &Layout::new(dir).locales,
        fluent::Options::default(),
        &mut report,
    )?;
    let config = Config::load(dir)?;
    let messages = messages(&outputs, &Layout::new(dir).locales, &config.source_locale);

    let options = Options {
        i18n_crate: match request.i18n_crate {
            Some(name) => Some(name.replace('-', "_")),
            None => package_name(&dir.join("Cargo.toml")),
        },
        without: None,
    };
    let changes = rewrite_all(&sources, &messages, &options, &mut report);
    for path in &manifests {
        report.extend(dependencies(path, &read(path)?));
    }

    if request.write {
        if let Some((path, _)) = outputs.iter().find(|(path, _)| path.exists()) {
            return Err(Error::Usage(format!(
                "{} already exists; mf2 convert never overwrites, so nothing was written \
                 (mf2 init --no-messages makes the i18n crate without starter messages)",
                path.display()
            )));
        }
        for (path, text) in &outputs {
            write(path, text)?;
        }
        for change in &changes {
            write(&change.path, &change.after)?;
        }
    } else if request.format == Format::Text {
        for change in &changes {
            print!("{}", diff(app, change));
        }
        for (path, _) in &outputs {
            println!("would write {}", path.display());
        }
    }

    match request.format {
        Format::Text => {
            print!("{}", report.to_text());
            let entries: usize = report.entries.values().sum();
            println!(
                "mf2 convert: {entries} entries in {} locale(s), {} .mf2 file(s) {}; {} Rust file(s) {}; {} error(s), {} warning(s)",
                report.entries.len(),
                outputs.len(),
                if request.write { "written" } else { "to write" },
                changes.len(),
                if request.write {
                    "rewritten"
                } else {
                    "to rewrite (--write applies)"
                },
                report.errors(),
                report.warnings()
            );
        }
        Format::Json => println!("{}", report.to_json()),
    }
    if report.errors() > 0 {
        return Err(Error::Corpus);
    }
    Ok(())
}

/// Rewrites every file, adding what each found to `report`.
pub(crate) fn rewrite_all(
    sources: &[(PathBuf, String)],
    messages: &Messages,
    options: &Options,
    report: &mut Report,
) -> Vec<Change> {
    let mut changes = Vec::new();
    for (path, text) in sources {
        let rewritten = rewrite::rewrite(path, text, messages, options);
        report.extend(rewritten.findings);
        if let Some(after) = rewritten.text {
            changes.push(Change {
                path: path.clone(),
                before: text.clone(),
                after,
            });
        }
    }
    changes
}

/// The source locale's messages among §6.1's outputs, with their variables.
pub(crate) fn messages(outputs: &[(PathBuf, String)], locales: &Path, source: &str) -> Messages {
    let mut out = Messages::default();
    let dir = locales.join(source);
    for (path, text) in outputs {
        if path.parent() != Some(dir.as_path()) {
            continue;
        }
        let (resource, _) = mf2_resource::parse(text);
        for entry in resource.iter() {
            let value = entry.entry.value.to_string();
            let parsed = mf2_syntax::parse_model(&value);
            let variables: BTreeSet<String> = parsed
                .message
                .as_ref()
                .map(|m| {
                    mf2_syntax::analyze(m)
                        .externals
                        .iter()
                        .map(|n| n.nfc.to_string())
                        .collect()
                })
                .unwrap_or_default();
            out.0.insert(entry.id().to_string(), variables);
        }
    }
    out
}

/// `--dir`, when it lies strictly inside the application and so must not
/// be read as its source.
fn skipped_dir(app: &Path, dir: &Path) -> Option<PathBuf> {
    let app = std::fs::canonicalize(app).ok()?;
    let dir = std::fs::canonicalize(dir).ok()?;
    (dir != app && dir.starts_with(&app)).then_some(dir)
}

/// Every `.rs` and `Cargo.toml` under `dir`, but `target/`, hidden
/// directories and `skip`.
fn collect(
    dir: &Path,
    skip: Option<&Path>,
    rust: &mut Vec<PathBuf>,
    manifests: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(|source| Error::io(dir, source))? {
        let entry = entry.map_err(|source| Error::io(dir, source))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            let skipped = skip.is_some_and(|s| std::fs::canonicalize(&path).is_ok_and(|p| p == s));
            if name == "target" || name.starts_with('.') || skipped {
                continue;
            }
            collect(&path, skip, rust, manifests)?;
        } else if name == "Cargo.toml" {
            manifests.push(path);
        } else if path.extension().is_some_and(|e| e == "rs") {
            rust.push(path);
        }
    }
    Ok(())
}

/// The package name in a `Cargo.toml`, as a path segment.
fn package_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = text.parse().ok()?;
    let name = table.get("package")?.get("name")?.as_str()?;
    Some(name.replace('-', "_"))
}

/// Each line of a manifest naming `leptos-fluent` or `fluent-templates`.
pub(crate) fn dependencies(path: &Path, text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let code = line.split('#').next().unwrap_or("");
        let found = [
            "leptos-fluent",
            "leptos_fluent",
            "fluent-templates",
            "fluent_templates",
        ]
        .iter()
        .find_map(|name| code.find(name).map(|at| (name, at)));
        if let Some((name, at)) = found {
            out.push(Finding {
                code: Code::LfDependency,
                locale: String::new(),
                file: path.to_path_buf(),
                line: u32::try_from(n + 1).unwrap_or(u32::MAX),
                column: u32::try_from(code[..at].chars().count() + 1).unwrap_or(u32::MAX),
                id: None,
                message: format!(
                    "`{name}`: depend on the i18n crate instead, and forward its features"
                ),
            });
        }
    }
    out
}

/// A unified diff of one change, paths relative to the application.
fn diff(app: &Path, change: &Change) -> String {
    let name = change
        .path
        .strip_prefix(app)
        .unwrap_or(&change.path)
        .display()
        .to_string();
    similar::TextDiff::from_lines(&change.before, &change.after)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string()
}
