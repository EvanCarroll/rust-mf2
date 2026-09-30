//! `mf2 fmt`: `.mf2` resources, canonically.

use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use mf2_build::Layout;
use mf2_build::loader::resource;

use crate::error::{Error, Result, read, write};

/// `mf2 fmt`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// Say which files would change and exit 1 if any would, writing
    /// nothing — for CI.
    #[arg(long)]
    check: bool,
    /// The files or directories to format; the corpus's `locales/` by
    /// default.
    #[arg(value_name = "PATH")]
    pub(crate) paths: Vec<PathBuf>,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let roots: Vec<PathBuf> = if args.paths.is_empty() {
        vec![Layout::new(dir).locales]
    } else {
        args.paths.clone()
    };
    let mut files = Vec::new();
    for root in &roots {
        collect(root, &mut files)?;
    }
    if files.is_empty() {
        return Err(Error::Usage(format!(
            "no .mf2 files under {}",
            roots
                .iter()
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let mut changed = Vec::new();
    let mut refused = Vec::new();
    for path in &files {
        let text = read(path)?;
        match resource::format(&text) {
            // A file the parser could not fully read is never rewritten:
            // `mf2 check` says what is wrong with it first.
            None => refused.push(path.clone()),
            Some(formatted) if formatted == text => {}
            Some(formatted) => {
                changed.push(path.clone());
                if !args.check {
                    write(path, formatted)?;
                }
            }
        }
    }

    for path in &refused {
        eprintln!(
            "mf2 fmt: {}: has a syntax error; left alone",
            path.display()
        );
    }
    for path in &changed {
        println!(
            "{}{}",
            if args.check { "would format " } else { "" },
            path.display()
        );
    }
    println!(
        "mf2 fmt: {} of {} file(s) {}",
        changed.len(),
        files.len(),
        if args.check {
            "would change"
        } else {
            "rewritten"
        }
    );
    if !refused.is_empty() || (args.check && !changed.is_empty()) {
        return Err(Error::Corpus);
    }
    Ok(())
}

/// Every `.mf2` file under `root`, sorted.
fn collect(root: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    if root.is_file() {
        into.push(root.to_path_buf());
        return Ok(());
    }
    let entries = std::fs::read_dir(root).map_err(|source| Error::io(root, source))?;
    let mut here = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(root, source))?;
        let path = entry.path();
        if path.is_dir() {
            collect(&path, into)?;
        } else if path.extension().is_some_and(|e| e == "mf2") {
            here.push(path);
        }
    }
    here.sort();
    into.extend(here);
    Ok(())
}
