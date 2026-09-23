//! `mf2 compile`: the manifest, the catalogs and the generated module,
//! without cargo — for a static host, a CSR build, or looking at what a
//! change did to the bytes.

use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use mf2_build::{Build, Config};

use crate::FeatureArgs;
use crate::error::{Error, Result};

/// `mf2 compile`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    #[command(flatten)]
    features: FeatureArgs,
    /// Where to write the manifest, the catalogs and the generated module.
    #[arg(long, short, value_name = "DIR", default_value = "dist")]
    out: PathBuf,
    /// Instead, write only what a static host serves: the catalogs and
    /// `index.json`, which a client-only application reads to find them
    /// (a trunk hook points this at its staging directory's `i18n/`).
    #[arg(long, value_name = "DIR", conflicts_with = "out")]
    site: Option<PathBuf>,
    /// The crate path the generated module re-exports as `__mf2`.
    #[arg(long, value_name = "PATH", default_value = "::mf2")]
    facade: String,
    /// Print what was written, one path per line.
    #[arg(long, short)]
    verbose: bool,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let build = Build::at(dir, &args.out)
        .config(config)
        .features(args.features.features())
        .facade(&args.facade);
    // A site gets the catalogs and the index only, so the build itself
    // writes nothing (`check` runs every stage but the write).
    let outcome = if args.site.is_some() {
        build.check()?
    } else {
        build.run()?
    };
    print!("{}", outcome.report.to_text());
    if !outcome.report.is_clean() {
        return Err(Error::Corpus);
    }
    let (to, written, removed) = match &args.site {
        Some(site) => {
            let published = outcome.publish(site)?;
            (site, published.written, published.removed)
        }
        None => (&args.out, outcome.written.clone(), outcome.removed.clone()),
    };
    if args.verbose {
        for path in &written {
            println!("{}", path.display());
        }
        for path in &removed {
            println!("removed {}", path.display());
        }
    }
    println!(
        "mf2 compile: {} messages, {} locales to {} ({} file(s) changed)",
        outcome.manifest.ids.len(),
        outcome.locales.len(),
        to.display(),
        written.len()
    );
    Ok(())
}
