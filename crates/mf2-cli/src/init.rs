//! `mf2 init`: the i18n crate an application adds once
//! (`plans/05-tooling.md` §4).

use std::path::Path;

use clap::Args as ClapArgs;
use mf2_build::Config;

use crate::error::{Error, Result, write};

/// `mf2 init`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// The crate's name.
    #[arg(long, value_name = "NAME", default_value = "my-app-i18n")]
    name: String,
    /// The locale the manifest is built from.
    #[arg(long, value_name = "TAG", default_value = "en")]
    source_locale: String,
    /// Further locales to scaffold.
    #[arg(long, value_name = "TAG")]
    locale: Vec<String>,
    /// Overwrite files that are already there.
    #[arg(long)]
    force: bool,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let mut files: Vec<(std::path::PathBuf, String)> = Vec::new();

    let mut config = Config::default();
    config.source_locale.clone_from(&args.source_locale);
    files.push((dir.join("mf2.toml"), config.to_toml()));
    files.push((dir.join("Cargo.toml"), cargo_toml(&args.name)));
    files.push((dir.join("build.rs"), BUILD_RS.to_owned()));
    files.push((dir.join("src/lib.rs"), LIB_RS.to_owned()));

    let mut locales = vec![args.source_locale.clone()];
    locales.extend(args.locale.iter().cloned());
    locales.dedup();
    for (i, tag) in locales.iter().enumerate() {
        let body = if i == 0 {
            format!("@locale {tag}\n---\n\nhello = Hello!\n")
        } else {
            // A translation starts empty: every id falls back until someone
            // translates it, and `mf2 check` says how many.
            format!("@locale {tag}\n---\n")
        };
        files.push((dir.join("locales").join(tag).join("main.mf2"), body));
    }

    let existing: Vec<&std::path::PathBuf> = files
        .iter()
        .map(|(path, _)| path)
        .filter(|path| path.exists())
        .collect();
    if !existing.is_empty() && !args.force {
        return Err(Error::Usage(format!(
            "{} file(s) are already there; --force overwrites them:\n  {}",
            existing.len(),
            existing
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n  ")
        )));
    }
    for (path, body) in &files {
        write(path, body)?;
        println!("{}", path.display());
    }
    println!(
        "\nmf2 init: {} in {}. Next:\n\
         \x20 1. add it to the workspace and to the application's dependencies;\n\
         \x20 2. call `{}::tr!(\"id\", name = value)` from anywhere that depends on it;\n\
         \x20 3. add this to the application's [package.metadata.leptos], so that\n\
         \x20    `cargo leptos watch` sees a translation change:\n\
         \x20      watch-additional-files = [\"{}/locales\"]",
        args.name,
        dir.display(),
        args.name.replace('-', "_"),
        dir.display()
    );
    Ok(())
}

fn cargo_toml(name: &str) -> String {
    format!(
        "[package]\n\
         name = {name:?}\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\
         \n\
         # The feature set is declared here, once, and the application forwards\n\
         # it from both its `ssr` and its `hydrate` builds, so the server and\n\
         # the client always have the same functions (plans/05-tooling.md §3.1).\n\
         [features]\n\
         default = []\n\
         ssr = [\"mf2/host-std\"]\n\
         hydrate = [\"mf2/host-web\"]\n\
         fn-number = [\"mf2/fn-number\"]\n\
         fn-datetime = [\"mf2/fn-datetime\"]\n\
         datetime-icu = [\"mf2/datetime-icu\", \"mf2-build/icu-blob\"]\n\
         datetime-intl = [\"mf2/datetime-intl\"]\n\
         intl = [\"mf2/intl\"]\n\
         \n\
         [dependencies]\n\
         mf2 = \"0.1\"\n\
         \n\
         [build-dependencies]\n\
         mf2-build = \"0.1\"\n"
    )
}

const BUILD_RS: &str = "\
//! Parses locales/, writes the manifest and the catalogs to OUT_DIR, and
//! generates the module src/lib.rs includes (plans/05-tooling.md §4).

fn main() {
    let outcome = match mf2_build::Build::new().and_then(|build| build.emit_cargo(true).run()) {
        Ok(outcome) => outcome,
        Err(e) => {
            println!(\"cargo::error={e}\");
            std::process::exit(1);
        }
    };
    if let Err(e) = outcome.into_result() {
        println!(\"cargo::error={e}\");
        std::process::exit(1);
    }
}
";

const LIB_RS: &str = "\
//! The application's messages. Everything in here is generated: edit
//! `locales/` instead.
//!
//! This brings in `tr!` and `msg_id!` as well. Every *other* crate calls them
//! as `<this crate>::tr!(\"id\", name = value)`; inside this one they are
//! called unqualified, because a `macro_export` macro that arrives through a
//! macro expansion cannot be named by an absolute path in its own crate
//! (rust-lang/rust#52234).

mf2::include_generated!();
";
