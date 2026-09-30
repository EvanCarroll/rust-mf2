//! `mf2` — the command line over `mf2-build`.
//!
//! Everything it does, a `build.rs` does too; the difference is that these
//! run without cargo, so a translator, a CI job or a static site can use
//! them.
//!
//! | Command | Does |
//! |---|---|
//! | `init` | a starter: `--cli` or `--tui` makes a native application, or adds translations to a crate; without either, a web application's translation crate |
//! | `check` | every lint, with `--format json` for CI |
//! | `compile` | the manifest, the catalogs and the generated module, without cargo |
//! | `fmt` | canonical `.mf2` resources (`--check` to only say which differ) |
//! | `stats` | coverage, catalog sizes raw/gz/br, the locale data entry by entry, the pins |
//! | `dump` | a catalog back to MF2 source or data-model JSON |
//! | `pseudo` | `en-XA` and `ar-XB` from the source locale |
//! | `export` / `import` | flat JSON, which every translation-management system speaks, and XLIFF 2 |
//! | `watch` | recompile when a locale file changes |
//! | `convert` | a one-shot migration from Fluent `.ftl` files, or from `leptos-fluent` (messages and call sites) |
//!
//! Every command, its flags and its report codes: the
//! [command-line chapter](https://evancarroll.github.io/rust-mf2/command-line.html)
//! of the [Rust MF2 book](https://evancarroll.github.io/rust-mf2/).

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
#[cfg(test)]
mod listing;
mod pseudo;
mod stats;
mod watch;
#[cfg(all(test, mf2_workspace))]
mod workspace_tests;

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
    /// application turns them on for `mf2`.
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
    /// Start: `--cli`, `--tui`, `--ssr`, `--islands` or `--csr` makes an
    /// application, or adds translations to the crate there.
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
    /// Write one locale as flat JSON, or as an XLIFF 2 document against the
    /// source locale.
    Export(exchange::ExportArgs),
    /// Read a locale back from flat JSON or an XLIFF 2 document; write
    /// nothing if that would bring an error `check` reports.
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

    #[test]
    fn the_help_summary_names_every_command() {
        let command = Cli::command();
        let about = command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default();
        for sub in command.get_subcommands() {
            let name = sub.get_name();
            let named = match name {
                "export" | "import" => about.contains("export/import"),
                _ => about.contains(name),
            };
            assert!(
                named,
                "`mf2 --help`'s summary does not name `{name}`: {about}"
            );
        }
    }
}
