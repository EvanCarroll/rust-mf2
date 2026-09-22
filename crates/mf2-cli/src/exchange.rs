//! `mf2 export` / `mf2 import`: flat JSON, the shape every
//! translation-management system speaks.
//!
//! Export writes `{id: source}` sorted by id. Import reads one back into the
//! container the locale already uses: a `.mf2` resource keeps its sections,
//! comments and properties, and only the values change — a translator's
//! round trip must not throw away the context the source carries.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use mf2_build::config::Layout;
use mf2_build::loader::{Loader, json, resource};
use mf2_resource::{parse, serialize_with};

use crate::error::{Error, Result, read, write};

/// `mf2 export`.
#[derive(Debug, ClapArgs)]
pub(crate) struct ExportArgs {
    /// The locale to write.
    #[arg(value_name = "LOCALE")]
    locale: String,
    /// Where to write it; standard output by default.
    #[arg(long, short, value_name = "FILE")]
    out: Option<PathBuf>,
}

/// `mf2 import`.
#[derive(Debug, ClapArgs)]
pub(crate) struct ImportArgs {
    /// The locale to read into.
    #[arg(value_name = "LOCALE")]
    locale: String,
    /// The flat JSON to read.
    #[arg(value_name = "FILE")]
    file: PathBuf,
    /// Say what would change and write nothing.
    #[arg(long)]
    dry_run: bool,
}

pub(crate) fn export(dir: &Path, args: &ExportArgs) -> Result<()> {
    let layout = Layout::new(dir);
    let path = locale_path(&layout, &args.locale)?;
    let loaded = mf2_build::loader::for_path(&path).load(&path)?;
    let text = json::write(
        loaded
            .records
            .iter()
            .map(|r| (r.id.as_str(), r.source.as_str())),
    );
    match &args.out {
        Some(file) => {
            write(file, &text)?;
            eprintln!(
                "mf2 export: {} messages of {} to {}",
                loaded.records.len(),
                args.locale,
                file.display()
            );
        }
        None => print!("{text}"),
    }
    Ok(())
}

pub(crate) fn import(dir: &Path, args: &ImportArgs) -> Result<()> {
    let layout = Layout::new(dir);
    let incoming: BTreeMap<String, String> = json::read(&read(&args.file)?)
        .map_err(|e| Error::Usage(format!("{}: {}", args.file.display(), e.message)))?
        .into_iter()
        .map(|pair| (pair.id, pair.source))
        .collect();

    let path = locale_path(&layout, &args.locale)?;
    if path.is_file() {
        // A flat JSON locale: the file *is* the values.
        let text = json::write(
            incoming
                .iter()
                .map(|(id, source)| (id.as_str(), source.as_str())),
        );
        return finish(&path, &text, args.dry_run, incoming.len(), 0);
    }

    // A resource locale: rewrite the values in place, leaving every section,
    // comment and property as it stands.
    let loaded = resource::Resources.load(&path)?;
    let known: BTreeSet<&str> = loaded.records.iter().map(|r| r.id.as_str()).collect();
    let unknown: Vec<&str> = incoming
        .keys()
        .filter(|id| !known.contains(id.as_str()))
        .map(String::as_str)
        .collect();
    let mut changed = 0usize;
    for file in &loaded.files {
        let (resource, diagnostics) = parse(&file.text);
        if !diagnostics.is_empty() {
            return Err(Error::Usage(format!(
                "{}: has a syntax error; run `mf2 check` first",
                file.path.display()
            )));
        }
        let out = resource.map_values(|info, value| {
            let id = info.full_id().to_string();
            match incoming.get(&id) {
                Some(source) if *source != value => {
                    changed += 1;
                    std::borrow::Cow::Owned(source.clone())
                }
                _ => value,
            }
        });
        let text = serialize_with(&out, &resource::FMT_STYLE)
            .map_err(|e| Error::Usage(format!("{}: {e}", file.path.display())))?;
        if text != file.text {
            if args.dry_run {
                println!("would rewrite {}", file.path.display());
            } else {
                write(&file.path, &text)?;
            }
        }
    }
    // An id the locale does not have needs a section to go in, and which one
    // is the translator's decision, not this command's: it is reported, not
    // invented.
    if !unknown.is_empty() {
        eprintln!(
            "mf2 import: {} id(s) are not in {} and were left out: {}",
            unknown.len(),
            args.locale,
            unknown
                .iter()
                .take(5)
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    eprintln!(
        "mf2 import: {changed} message(s) {} in {}",
        if args.dry_run {
            "would change"
        } else {
            "changed"
        },
        args.locale
    );
    Ok(())
}

fn finish(path: &Path, text: &str, dry_run: bool, count: usize, _changed: usize) -> Result<()> {
    if dry_run {
        println!("would rewrite {}", path.display());
    } else {
        write(path, text)?;
    }
    eprintln!("mf2 import: {count} message(s) into {}", path.display());
    Ok(())
}

/// Where a locale lives: `locales/<tag>/` or `locales/<tag>.json`.
fn locale_path(layout: &Layout, tag: &str) -> Result<PathBuf> {
    let dir = layout.locales.join(tag);
    if dir.is_dir() {
        return Ok(dir);
    }
    let file = layout.locales.join(format!("{tag}.json"));
    if file.is_file() {
        return Ok(file);
    }
    Err(Error::Usage(format!(
        "{}: no locale {tag} (expected {} or {})",
        layout.locales.display(),
        dir.display(),
        file.display()
    )))
}
