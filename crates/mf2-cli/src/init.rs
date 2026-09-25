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
    /// Leave out the starter `locales/<tag>/main.mf2` files, for messages
    /// that come from elsewhere (`mf2 convert`, `mf2 import`).
    #[arg(long)]
    no_messages: bool,
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
    for (i, tag) in locales.iter().enumerate().filter(|_| !args.no_messages) {
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
    let krate = args.name.replace('-', "_");
    println!(
        "\nmf2 init: {name} in {dir}. Next:\n\
         \x20 1. add it to the workspace and to the application's dependencies, and\n\
         \x20    forward the application's `ssr`, `hydrate` or `csr` feature to it;\n\
         \x20 2. install it once on each side: `mf2_axum::install({krate}::setup(),\n\
         \x20    {krate}::CATALOGS)` in the server's `main`, and\n\
         \x20    `leptos_mf2::install({krate}::setup())` before the client boots;\n\
         \x20 3. call `{krate}::tr!(\"id\", name = value)` from anywhere that depends on it;\n\
         \x20 4. add this to the application's [package.metadata.leptos], so that\n\
         \x20    `cargo leptos watch` sees a translation change:\n\
         \x20      watch-additional-files = [\"{dir}/locales\"]\n\
         \x20 5. only for a client-only application (no server to embed the\n\
         \x20    catalogs in): emit `mf2_build::Emit::Module` in build.rs, and\n\
         \x20    publish the catalogs beside the wasm with\n\
         \x20      mf2 -C {dir} compile --site <site>/i18n\n\
         \x20    which builds them for this crate's features as cargo resolves them.",
        name = args.name,
        dir = dir.display(),
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
         # The functions a message may use are this crate's features, declared\n\
         # once: the application's server and client builds both get them, so\n\
         # the two always format alike. Turn on what the corpus needs.\n\
         [features]\n\
         default = []\n\
         # The application forwards exactly one of these from its own build.\n\
         ssr = [\"mf2/host-std\", \"mf2/ssr\"]\n\
         hydrate = [\"mf2/host-web\", \"mf2/hydrate\"]\n\
         csr = [\"mf2/host-web\", \"mf2/csr\"]\n\
         fn-number = [\"mf2/fn-number\"]\n\
         fn-datetime = [\"mf2/fn-datetime\"]\n\
         datetime-icu = [\"fn-datetime\", \"mf2/datetime-icu\", \"mf2-build/icu-blob\"]\n\
         datetime-intl = [\"fn-datetime\", \"mf2/datetime-intl\"]\n\
         intl = [\"mf2/intl\"]\n\
         \n\
         [dependencies]\n\
         mf2 = \"1\"\n\
         \n\
         [build-dependencies]\n\
         mf2-build = \"1\"\n"
    )
}

const BUILD_RS: &str = "\
//! Parses locales/, writes the manifest and the catalogs to OUT_DIR, and
//! generates the module src/lib.rs includes.

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

/// What the application installs once on each side: the registry, the host,
/// the manifest hash and the locale table the build generated.
#[cfg(any(feature = \"ssr\", feature = \"hydrate\", feature = \"csr\"))]
#[must_use]
pub fn setup() -> mf2::leptos_mf2::Setup {
    mf2::leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}
";
