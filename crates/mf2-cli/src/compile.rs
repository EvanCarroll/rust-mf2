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
    /// The crate path the generated module re-exports as `__mf2`.
    #[arg(long, value_name = "PATH", default_value = "::mf2")]
    facade: String,
    /// Print what was written, one path per line.
    #[arg(long, short)]
    verbose: bool,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let outcome = Build::at(dir, &args.out)
        .config(config)
        .features(args.features.features())
        .facade(&args.facade)
        .run()?;
    print!("{}", outcome.report.to_text());
    if !outcome.report.is_clean() {
        return Err(Error::Corpus);
    }
    if args.verbose {
        for path in &outcome.written {
            println!("{}", path.display());
        }
        for path in &outcome.removed {
            println!("removed {}", path.display());
        }
    }
    println!(
        "mf2 compile: {} messages, {} locales to {} ({} file(s) changed)",
        outcome.manifest.ids.len(),
        outcome.locales.len(),
        args.out.display(),
        outcome.written.len()
    );
    Ok(())
}
