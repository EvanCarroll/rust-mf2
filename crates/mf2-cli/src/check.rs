//! `mf2 check`: every lint over the corpus.

use std::path::Path;

use clap::Args as ClapArgs;
use mf2_build::{Build, Config};

use crate::cargo::resolved_features;
use crate::error::{Error, Result};
use crate::{FeatureArgs, Format};

/// `mf2 check`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    // Without `--features`, mf2's, as cargo resolves them for this crate.
    #[command(flatten)]
    features: FeatureArgs,
    /// How to report.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Fail on warnings too.
    #[arg(long)]
    deny_warnings: bool,
    /// Where the application's Rust sources are, for `unused-id`: an id no
    /// `tr!` in them names. Repeat for several directories.
    #[arg(long, value_name = "DIR")]
    pub(crate) src: Vec<std::path::PathBuf>,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let mut outcome = Build::at(dir, std::env::temp_dir().join("mf2-check"))
        .config(config.clone())
        .features(features(dir, &args.features))
        .check()?;
    if !args.src.is_empty() {
        unused_ids(&mut outcome, &config, &args.src)?;
    }
    match args.format {
        Format::Text => {
            print!("{}", outcome.report.to_text());
            let (errors, warnings) = (outcome.report.errors(), outcome.report.warnings());
            if errors == 0 && warnings == 0 {
                println!(
                    "mf2 check: {} messages in {} locales, nothing to report",
                    outcome.manifest.ids.len(),
                    outcome.locales.len()
                );
            } else {
                println!("mf2 check: {errors} error(s), {warnings} warning(s)");
            }
        }
        Format::Json => println!("{}", outcome.report.to_json()),
    }
    if outcome.report.errors() > 0 || (args.deny_warnings && outcome.report.warnings() > 0) {
        return Err(Error::Corpus);
    }
    Ok(())
}

/// The features to check with: `--features` if given, else those cargo
/// resolves for the crate's `mf2` — the build reads the same through
/// `links`, so a check with others warns where the build does not and fails
/// where it succeeds. Without an answer from cargo, every function's, so
/// that no function is reported as gated when it may not be, and one line
/// says so (on stderr, so that `--format json` stays one document). `mf2
/// import` checks what it would write with the same, and `mf2 stats`
/// counts what the build ships.
pub(crate) fn features(dir: &Path, args: &FeatureArgs) -> mf2_build::Features {
    if let Some(given) = args.given() {
        return given;
    }
    if let Ok((_, resolved)) = resolved_features(dir, true) {
        return resolved;
    }
    eprintln!(
        "note: cargo could not say which of mf2's features this crate has, so every \
         function is assumed on; name them with --features (for example \
         --features fn-number)"
    );
    mf2_build::Features::from_names(["fn-number", "fn-datetime"])
}

/// The `unused-id` lint needs what the application's sources say, which only
/// the command line knows; the check itself is `mf2-build`'s.
fn unused_ids(
    outcome: &mut mf2_build::Outcome,
    config: &Config,
    src: &[std::path::PathBuf],
) -> Result<()> {
    let mut sources = String::new();
    for dir in src {
        collect_rust(dir, &mut sources)?;
    }
    // The generated module names the languages' names itself: the switcher
    // and `Locale::name()` use them. The pseudo-locales' are the build's too.
    let ids: Vec<String> = outcome
        .manifest
        .ids
        .iter()
        .filter(|id| !outcome.added.contains(id) && !outcome.used.contains(id))
        .cloned()
        .collect();
    mf2_build::check::unused_ids(
        &ids,
        &sources,
        &outcome.source_locale.clone(),
        &outcome.defined,
        &src[0],
        config,
        &mut outcome.report,
    );
    Ok(())
}

fn collect_rust(dir: &Path, into: &mut String) -> Result<()> {
    if dir.is_file() {
        into.push_str(&crate::error::read(dir)?);
        into.push('\n');
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|source| Error::io(dir, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(dir, source))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust(&path, into)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push_str(&crate::error::read(&path)?);
            into.push('\n');
        }
    }
    Ok(())
}
