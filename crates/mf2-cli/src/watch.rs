//! `mf2 watch`: recompile whenever a locale file changes.
//!
//! It polls modification times rather than subscribing to the operating
//! system's file events: a corpus is a few hundred files, a poll costs a
//! fraction of a millisecond, and nothing here has to know about inotify,
//! kqueue, `ReadDirectoryChangesW` or the editors that write through a
//! temporary file. Pushing the new catalog to open pages is `mf2-axum`'s dev
//! mode (P7).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clap::Args as ClapArgs;
use mf2_build::config::Layout;
use mf2_build::{Build, Config};

use crate::FeatureArgs;
use crate::error::{Error, Result};

/// `mf2 watch`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    #[command(flatten)]
    features: FeatureArgs,
    /// Where to write the outputs.
    #[arg(long, short, value_name = "DIR", default_value = "dist")]
    out: PathBuf,
    /// How often to look, in milliseconds.
    #[arg(long, value_name = "MS", default_value_t = 300)]
    interval: u64,
    /// Stop after this many rebuilds (0 = never stop). For tests.
    #[arg(long, default_value_t = 0)]
    max_rebuilds: u32,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let layout = Layout::new(dir);
    let interval = Duration::from_millis(args.interval.max(20));
    println!(
        "mf2 watch: {} → {} (every {} ms; ^C to stop)",
        layout.locales.display(),
        args.out.display(),
        interval.as_millis()
    );
    let mut seen = stamps(&layout, dir);
    build(dir, args)?;
    let mut rebuilds = 0u32;
    loop {
        std::thread::sleep(interval);
        let now = stamps(&layout, dir);
        if now == seen {
            continue;
        }
        for path in changed(&seen, &now) {
            println!("mf2 watch: {}", path.display());
        }
        seen = now;
        // A build that fails prints its report and the watch goes on: the
        // next save is usually the fix.
        if let Err(e) = build(dir, args) {
            eprintln!("mf2 watch: {e}");
        }
        rebuilds += 1;
        if args.max_rebuilds != 0 && rebuilds >= args.max_rebuilds {
            return Ok(());
        }
    }
}

fn build(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let outcome = Build::at(dir, &args.out)
        .config(config)
        .features(args.features.features())
        .run()?;
    print!("{}", outcome.report.to_text());
    if !outcome.report.is_clean() {
        return Err(Error::Corpus);
    }
    println!(
        "mf2 watch: {} messages, {} locales, {} file(s) changed",
        outcome.manifest.ids.len(),
        outcome.locales.len(),
        outcome.written.len()
    );
    Ok(())
}

/// The modification time of every file a build reads.
fn stamps(layout: &Layout, dir: &Path) -> BTreeMap<PathBuf, SystemTime> {
    let mut out = BTreeMap::new();
    let config = dir.join(mf2_build::config::FILE_NAME);
    if let Ok(meta) = std::fs::metadata(&config)
        && let Ok(time) = meta.modified()
    {
        out.insert(config, time);
    }
    walk(&layout.locales, &mut out);
    out
}

fn walk(dir: &Path, into: &mut BTreeMap<PathBuf, SystemTime>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, into);
        } else if let Ok(meta) = entry.metadata()
            && let Ok(time) = meta.modified()
        {
            into.insert(path, time);
        }
    }
}

/// What differs between two polls: written, added or removed.
fn changed(
    before: &BTreeMap<PathBuf, SystemTime>,
    now: &BTreeMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = now
        .iter()
        .filter(|(path, time)| before.get(*path) != Some(time))
        .map(|(path, _)| path.clone())
        .collect();
    out.extend(
        before
            .keys()
            .filter(|path| !now.contains_key(*path))
            .cloned(),
    );
    out.sort();
    out.dedup();
    out
}
