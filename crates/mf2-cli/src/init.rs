//! `mf2 init`: a starter (`plans/05-tooling.md` §6.4). `--cli` and `--tui`
//! make a complete native application in an empty or missing directory — the
//! shapes of `plans/19-native-and-terminal.md` §1.1 and §1.2 — or add
//! translations to the crate already there. Without a mode, it writes the
//! translation crate the web pages use, until the web starters replace it.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args as ClapArgs;
use mf2_build::Config;

use crate::error::{Error, Result, write};

/// The major version of `mf2` and `mf2-build` a starter names.
const MAJOR: &str = "2";

/// `mf2 init`.
// Each bool is a flag of its own on the command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// A command-line application: `clap`, `--lang`, a plural, `println!`.
    #[arg(long, conflicts_with = "tui")]
    cli: bool,
    /// A Ratatui terminal UI: a table, a language menu, a live switch.
    #[arg(long)]
    tui: bool,
    /// With `--cli` or `--tui`: the new application's directory (empty or
    /// missing), or a crate to add translations to. The current directory
    /// by default.
    #[arg(value_name = "DIR")]
    path: Option<PathBuf>,
    /// The crate's name: a new application's is its directory's.
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
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

/// What `--cli` and `--tui` make.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Cli,
    Tui,
}

impl Mode {
    /// The features of `mf2` the application needs.
    fn features(self) -> &'static str {
        match self {
            Mode::Cli => "native",
            Mode::Tui => "native,ratatui",
        }
    }

    fn what(self) -> &'static str {
        match self {
            Mode::Cli => "a command-line application",
            Mode::Tui => "a terminal UI",
        }
    }
}

/// The build script every starter writes.
const BUILD_RS: &str = "fn main() {\n    mf2_build::run();\n}\n";

/// The tip for a manifest `init` does not write.
const BUILD_OVERRIDE: &str = "[profile.dev.build-override]\nopt-level = 2\n";

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let mode = match (args.cli, args.tui) {
        (true, _) => Mode::Cli,
        (_, true) => Mode::Tui,
        _ if args.path.is_some() => {
            return Err(Error::Usage(
                "a directory is for --cli or --tui; the translation crate is written in -C's"
                    .into(),
            ));
        }
        _ => return translation_crate(dir, args),
    };
    let target = match &args.path {
        Some(path) => dir.join(path),
        None => dir.to_owned(),
    };
    if target.join("Cargo.toml").is_file() {
        existing_crate(&target, mode, args)
    } else {
        new_application(&target, mode, args)
    }
}

/// Writes `files`, refusing to overwrite any without `--force`.
fn write_all(files: &[(PathBuf, String)], force: bool) -> Result<()> {
    let existing: Vec<String> = files
        .iter()
        .filter(|(path, _)| path.exists())
        .map(|(path, _)| path.display().to_string())
        .collect();
    if !existing.is_empty() && !force {
        return Err(Error::Usage(format!(
            "{} file(s) are already there; --force overwrites them:\n  {}",
            existing.len(),
            existing.join("\n  ")
        )));
    }
    for (path, body) in files {
        write(path, body)?;
        println!("{}", path.display());
    }
    Ok(())
}

/// A complete application, in an empty or missing directory.
fn new_application(target: &Path, mode: Mode, args: &Args) -> Result<()> {
    if target.is_file() {
        return Err(Error::Usage(format!("{} is a file", target.display())));
    }
    if !args.force
        && let Ok(mut entries) = std::fs::read_dir(target)
        && entries.next().is_some()
    {
        return Err(Error::Usage(format!(
            "{} is neither empty nor a crate (no Cargo.toml); --force writes the application into it",
            target.display()
        )));
    }
    if args.source_locale != "en" || !args.locale.is_empty() || args.no_messages {
        return Err(Error::Usage(
            "a new application starts from its messages in en and fr; add a language to \
             locales/ once it is made (--source-locale, --locale and --no-messages are for \
             an existing crate)"
                .into(),
        ));
    }
    let name = if let Some(name) = &args.name {
        name.clone()
    } else {
        let full = std::path::absolute(target).map_err(|source| Error::io(target, source))?;
        full.components()
            .rev()
            .find_map(|c| match c {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Usage("name the application with --name".into()))?
    };
    let (en, fr, main) = match mode {
        Mode::Cli => (
            include_str!("../starters/cli/en.mf2"),
            include_str!("../starters/cli/fr.mf2"),
            include_str!("../starters/cli/main.rs"),
        ),
        Mode::Tui => (
            include_str!("../starters/tui/en.mf2"),
            include_str!("../starters/tui/fr.mf2"),
            include_str!("../starters/tui/main.rs"),
        ),
    };
    let mut files = vec![
        (target.join("Cargo.toml"), application_toml(&name, mode)),
        (target.join("build.rs"), BUILD_RS.to_owned()),
        (target.join("locales/en/main.mf2"), en.to_owned()),
        (target.join("locales/fr/main.mf2"), fr.to_owned()),
        (target.join("src/main.rs"), main.to_owned()),
    ];
    if mode == Mode::Tui {
        files.push((
            target.join("src/ui.rs"),
            include_str!("../starters/tui/ui.rs").to_owned(),
        ));
    }
    write_all(&files, args.force)?;
    let try_it = match mode {
        Mode::Cli => "cargo run -- --lang fr",
        Mode::Tui => "cargo run    (1 and 2 switch the language, q quits)",
    };
    println!(
        "\nmf2 init: {what} in {dir}. Try it:\n\
         \x20 cd {dir}\n\
         \x20 {try_it}\n\
         The messages are in locales/; a change to one rebuilds the application.",
        what = mode.what(),
        dir = target.display(),
    );
    Ok(())
}

/// A new application's manifest: 19 §1.1's or §1.2's, with the
/// build-override that keeps an edit to a message quick.
fn application_toml(name: &str, mode: Mode) -> String {
    let (mf2, ratatui) = match mode {
        Mode::Cli => ("\"native\", \"fn-number\"", ""),
        Mode::Tui => ("\"ratatui\", \"fn-number\"", "ratatui = \"0.30\"\n"),
    };
    format!(
        "[package]\n\
         name = {name:?}\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\
         \n\
         [dependencies]\n\
         clap = {{ version = \"4\", features = [\"derive\"] }}\n\
         mf2 = {{ version = \"{MAJOR}\", features = [{mf2}] }}\n\
         {ratatui}\
         \n\
         [build-dependencies]\n\
         mf2-build = \"{MAJOR}\"\n\
         \n\
         # The build script compiles the messages again after every edit to\n\
         # them: built optimized, it does so faster.\n\
         {BUILD_OVERRIDE}"
    )
}

/// Translations for the crate in `target`: a build script, the source
/// locale's messages, and `mf2` and `mf2-build` added with `cargo add`.
fn existing_crate(target: &Path, mode: Mode, args: &Args) -> Result<()> {
    let mut files = vec![(target.join("build.rs"), BUILD_RS.to_owned())];
    if args.source_locale != "en" {
        let mut config = Config::default();
        config.source_locale.clone_from(&args.source_locale);
        files.push((target.join("mf2.toml"), config.to_toml()));
    }
    files.extend(locale_files(target, args));
    if !args.force
        && let Some((path, _)) = files.iter().find(|(path, _)| path.exists())
    {
        return Err(Error::Usage(format!(
            "{} is already there; --force overwrites it",
            path.display()
        )));
    }
    cargo_add(
        target,
        &["add", &format!("mf2@{MAJOR}"), "-F", mode.features()],
    )?;
    cargo_add(target, &["add", "--build", &format!("mf2-build@{MAJOR}")])?;
    write_all(&files, args.force)?;
    let ratatui = match mode {
        Mode::Cli => "",
        Mode::Tui => {
            "\x20 4. a message is Ratatui text — `Line::from(tr!(\"hello\"))` — and\n\
             \x20    `mf2::ratatui::set_theme` says how its markup is drawn;\n"
        }
    };
    println!(
        "\nmf2 init: translations for the crate in {dir}. Next:\n\
         \x20 1. include the generated module once, at the crate root (src/main.rs\n\
         \x20    or src/lib.rs):\n\
         \x20      mf2::include_generated!();\n\
         \x20 2. at the start of `main`, load the embedded catalogs and take the\n\
         \x20    system's language: `install();`\n\
         \x20 3. call `tr!(\"hello\")`: at the root, after the include, it needs no\n\
         \x20    import; any other module writes `use crate::prelude::*;`\n\
         {ratatui}\
         Tip: the build script compiles the messages again after every edit to\n\
         them. Built optimized, it does so faster: add this to the workspace's\n\
         root Cargo.toml:\n\n{BUILD_OVERRIDE}",
        dir = target.display(),
    );
    Ok(())
}

/// Runs `cargo <args>` in `dir`: the cargo that ran us, if one did.
fn cargo_add(dir: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let command = format!("cargo {}", args.join(" "));
    eprintln!("{command}");
    let status = Command::new(cargo)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| Error::CargoAdd {
            dir: dir.to_owned(),
            command: command.clone(),
            message: e.to_string(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::CargoAdd {
            dir: dir.to_owned(),
            command,
            message: format!("exited with {status}"),
        })
    }
}

/// `locales/<tag>/main.mf2` for the source locale and each `--locale`,
/// unless `--no-messages`.
fn locale_files(dir: &Path, args: &Args) -> Vec<(PathBuf, String)> {
    if args.no_messages {
        return Vec::new();
    }
    let mut locales = vec![args.source_locale.clone()];
    locales.extend(args.locale.iter().cloned());
    locales.dedup();
    locales
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let body = if i == 0 {
                format!("@locale {tag}\n---\n\nhello = Hello!\n")
            } else {
                // A translation starts empty: every id falls back until
                // someone translates it, and `mf2 check` says how many.
                format!("@locale {tag}\n---\n")
            };
            (dir.join("locales").join(tag).join("main.mf2"), body)
        })
        .collect()
}

/// Without a mode: the translation crate of a web application.
fn translation_crate(dir: &Path, args: &Args) -> Result<()> {
    let name = args.name.as_deref().unwrap_or("my-app-i18n");
    let mut config = Config::default();
    config.source_locale.clone_from(&args.source_locale);
    let mut files = vec![
        (dir.join("mf2.toml"), config.to_toml()),
        (dir.join("Cargo.toml"), cargo_toml(name)),
        (dir.join("build.rs"), I18N_BUILD_RS.to_owned()),
        (dir.join("src/lib.rs"), LIB_RS.to_owned()),
    ];
    files.extend(locale_files(dir, args));
    write_all(&files, args.force)?;
    let krate = name.replace('-', "_");
    println!(
        "\nmf2 init: {name} in {dir}. Next:\n\
         \x20 1. add it to the workspace and to the application's dependencies, and\n\
         \x20    forward the application's `ssr`, `hydrate` or `csr` feature to it;\n\
         \x20 2. install it once on each side: `mf2_axum::install({krate}::setup(),\n\
         \x20    {krate}::CATALOGS)` in the server's `main`, and\n\
         \x20    `leptos_mf2::install({krate}::setup())` before the client boots;\n\
         \x20 3. from any crate that depends on it, call `{krate}::tr!(\"id\", name = value)`,\n\
         \x20    or write `use {krate}::prelude::*;` and call `tr!` itself;\n\
         \x20 4. add this to the application's [package.metadata.leptos], so that\n\
         \x20    `cargo leptos watch` sees a translation change:\n\
         \x20      watch-additional-files = [\"{dir}/locales\"]\n\
         \x20 5. only for a client-only application (no server to embed the\n\
         \x20    catalogs in): emit `mf2_build::Emit::Module` in build.rs, and\n\
         \x20    publish the catalogs beside the wasm with\n\
         \x20      mf2 -C {dir} compile --site <site>/i18n\n\
         \x20    which builds them for this crate's features as cargo resolves them.",
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

const I18N_BUILD_RS: &str = "\
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
//! This brings in `tr!` and `msg_id!` as well, and a `prelude` that holds
//! both. Another crate calls them as `<this crate>::tr!(\"id\", name = value)`,
//! or imports them with `use <this crate>::prelude::*;`; a module of this
//! crate imports them with `use crate::prelude::*;`.

mf2::include_generated!();
";
