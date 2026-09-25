//! `mf2` — the command line over `mf2-build` (`plans/05-tooling.md` §6).
//!
//! Everything it does, a `build.rs` does too; the difference is that these
//! run without cargo, so a translator, a CI job or a static site can use
//! them.
//!
//! | Command | Does |
//! |---|---|
//! | `init` | scaffold an i18n crate: `locales/`, `mf2.toml`, `build.rs`, `src/lib.rs` |
//! | `check` | every lint, with `--format json` for CI |
//! | `compile` | the manifest, the catalogs and the generated module, without cargo |
//! | `fmt` | canonical `.mf2` resources (`--check` to only say which differ) |
//! | `stats` | coverage, catalog sizes raw/gz/br, the locale data entry by entry, the pins |
//! | `dump` | a catalog back to MF2 source or data-model JSON |
//! | `pseudo` | `en-XA` and `ar-XB` from the source locale |
//! | `export` / `import` | flat JSON, which every translation-management system speaks |
//! | `watch` | recompile when a locale file changes |
//! | `convert` | a one-shot migration from Fluent `.ftl` files |

#![forbid(unsafe_code)]
// A command returns its error once, at the top; the variants stay readable.
#![allow(clippy::result_large_err)]

mod cargo;
mod check;
mod compile;
mod convert;
mod dump;
mod error;
mod exchange;
mod fmt;
mod init;
mod pseudo;
mod stats;
mod watch;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::error::{Error, Result};

/// How a command reports.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, ValueEnum)]
pub(crate) enum Format {
    /// One line per finding, for a person.
    #[default]
    Text,
    /// JSON, for CI.
    Json,
}

/// Unicode MessageFormat 2 for Leptos: the corpus tool.
#[derive(Debug, Parser)]
#[command(name = "mf2", version, about, long_about = None)]
struct Cli {
    /// The i18n crate: where `mf2.toml` and `locales/` are.
    #[arg(
        long,
        short = 'C',
        global = true,
        default_value = ".",
        value_name = "DIR"
    )]
    dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

/// The client feature set, which decides which functions exist.
#[derive(Debug, Args, Clone, Default)]
pub(crate) struct FeatureArgs {
    /// Comma-separated client features (`fn-number,fn-datetime`), as the
    /// application declares them on its i18n crate.
    #[arg(long, value_name = "LIST")]
    features: Option<String>,
}

impl FeatureArgs {
    /// What the arguments name; none if `--features` is absent.
    pub(crate) fn features(&self) -> mf2_build::Features {
        self.given().unwrap_or_default()
    }

    /// What `--features` names, if it was given at all.
    pub(crate) fn given(&self) -> Option<mf2_build::Features> {
        self.features.as_deref().map(mf2_build::Features::parse)
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold an i18n crate: `locales/<tag>/`, `mf2.toml`, `build.rs` and
    /// the `src/lib.rs` that includes what the build generates.
    Init(init::Args),
    /// Run every lint over the corpus; exit 1 if anything is an error.
    Check(check::Args),
    /// Write the manifest, one catalog per locale and the generated module,
    /// without cargo.
    Compile(compile::Args),
    /// Rewrite `.mf2` resources canonically.
    Fmt(fmt::Args),
    /// Coverage, catalog sizes, the locale data entry by entry, the pins.
    Stats(stats::Args),
    /// Decode a catalog back to MF2 source or data-model JSON.
    Dump(dump::Args),
    /// Write the pseudo-locales `en-XA` and `ar-XB` from the source locale.
    Pseudo(pseudo::Args),
    /// Write one locale as flat JSON.
    Export(exchange::ExportArgs),
    /// Read a locale back from flat JSON.
    Import(exchange::ImportArgs),
    /// Recompile whenever a locale file changes.
    Watch(watch::Args),
    /// Convert a Fluent project's `.ftl` files to `.mf2` resources, once;
    /// exit 1 if anything could not be converted.
    Convert(convert::Args),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Corpus) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("mf2: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Init(args) => init::run(&cli.dir, args),
        Command::Check(args) => check::run(&cli.dir, args),
        Command::Compile(args) => compile::run(&cli.dir, args),
        Command::Fmt(args) => fmt::run(&cli.dir, args),
        Command::Stats(args) => stats::run(&cli.dir, args),
        Command::Dump(args) => dump::run(args),
        Command::Pseudo(args) => pseudo::run(&cli.dir, args),
        Command::Export(args) => exchange::export(&cli.dir, args),
        Command::Import(args) => exchange::import(&cli.dir, args),
        Command::Watch(args) => watch::run(&cli.dir, args),
        Command::Convert(args) => convert::run(&cli.dir, args),
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_is_well_formed() {
        Cli::command().debug_assert();
    }
}
