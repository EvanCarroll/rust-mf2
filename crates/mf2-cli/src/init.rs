//! `mf2 init`: a starter (the tooling design §6.4). Each mode makes a
//! complete application in an empty or missing directory — `--cli` and
//! `--tui` the shapes of the native-and-terminal design §1.1 and §1.2,
//! `--ssr` §1.4's, `--islands` and `--csr` the delivery modes' — or adds
//! translations to the crate already there. Without a mode, it names the
//! five and changes nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args as ClapArgs;
use mf2_build::Config;

use crate::error::{Error, Result, write};

/// The major version of `mf2` and `mf2-build` a starter names.
const MAJOR: &str = "3";

/// `mf2 init`.
// Each bool is a flag of its own on the command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, ClapArgs)]
#[command(group = clap::ArgGroup::new("mode").multiple(false))]
pub(crate) struct Args {
    /// A command-line application: `clap`, `--lang`, a plural, `println!`.
    #[arg(long, group = "mode")]
    cli: bool,
    /// A Ratatui terminal UI: a table, a language menu, a live switch.
    #[arg(long, group = "mode")]
    tui: bool,
    /// A Leptos application rendered on the server and hydrated, built
    /// with cargo-leptos.
    #[arg(long, group = "mode")]
    ssr: bool,
    /// A Leptos islands application: only its islands run in the browser.
    #[arg(long, group = "mode")]
    islands: bool,
    /// A client-only Leptos application, built with Trunk.
    #[arg(long, group = "mode")]
    csr: bool,
    /// The new application's directory (empty or missing), or a crate to
    /// add translations to. The current directory by default.
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

/// What each mode flag makes.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Cli,
    Tui,
    Ssr,
    Islands,
    Csr,
}

impl Mode {
    fn of(args: &Args) -> Option<Mode> {
        [
            (args.cli, Mode::Cli),
            (args.tui, Mode::Tui),
            (args.ssr, Mode::Ssr),
            (args.islands, Mode::Islands),
            (args.csr, Mode::Csr),
        ]
        .into_iter()
        .find_map(|(on, mode)| on.then_some(mode))
    }

    /// The features of `mf2` the application needs: the one list both a
    /// new application's manifest and `cargo add` for an existing crate
    /// write. `ratatui` implies `native`; the server's and the client's
    /// modes (`ssr`, `hydrate`) are forwarded by the crate's own features.
    fn features(self) -> &'static [&'static str] {
        match self {
            Mode::Cli => &["native", "fn-number"],
            Mode::Tui => &["ratatui", "fn-number"],
            Mode::Ssr => &["leptos", "fn-number"],
            Mode::Islands => &["leptos", "fn-number", "static-locale"],
            Mode::Csr => &["leptos", "csr", "fn-number"],
        }
    }

    /// [`Mode::features`] as a manifest writes it: `"a", "b"`.
    fn features_toml(self) -> String {
        self.features()
            .iter()
            .map(|feature| format!("{feature:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn what(self) -> &'static str {
        match self {
            Mode::Cli => "a command-line application",
            Mode::Tui => "a terminal UI",
            Mode::Ssr => "a server-rendered Leptos application",
            Mode::Islands => "a Leptos islands application",
            Mode::Csr => "a client-only Leptos application",
        }
    }

    fn web(self) -> bool {
        matches!(self, Mode::Ssr | Mode::Islands | Mode::Csr)
    }
}

/// What `init` says without a mode.
const MODES: &str = "say what to make:
  --cli       a command-line application
  --tui       a Ratatui terminal UI
  --ssr       a Leptos application, rendered on the server and hydrated
  --islands   a Leptos islands application
  --csr       a client-only Leptos application
In an empty or missing directory, each makes a complete application; in a
crate, it adds translations to it.";

/// The build script every starter writes.
const BUILD_RS: &str = "fn main() {\n    mf2_build::run();\n}\n";

/// The tip for a manifest `init` does not write.
const BUILD_OVERRIDE: &str = "[profile.dev.build-override]\nopt-level = 2\n";

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let Some(mode) = Mode::of(args) else {
        return Err(Error::Usage(MODES.into()));
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
    let files = starter_files(target, &name, mode);
    write_all(&files, args.force)?;
    let try_it = match mode {
        Mode::Cli => "cargo run -- --lang fr",
        Mode::Tui => "cargo run    (1 and 2 switch the language, q quits)",
        Mode::Ssr | Mode::Islands => "cargo leptos watch    (then open http://127.0.0.1:3000)",
        Mode::Csr => "trunk serve    (with `mf2` on the PATH: Trunk.toml runs it)",
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

/// Every file of a new application.
fn starter_files(target: &Path, name: &str, mode: Mode) -> Vec<(PathBuf, String)> {
    let (en, fr) = match mode {
        Mode::Cli => (
            include_str!("../starters/cli/en.mf2"),
            include_str!("../starters/cli/fr.mf2"),
        ),
        Mode::Tui => (
            include_str!("../starters/tui/en.mf2"),
            include_str!("../starters/tui/fr.mf2"),
        ),
        Mode::Ssr | Mode::Islands | Mode::Csr => (
            include_str!("../starters/web/en.mf2"),
            include_str!("../starters/web/fr.mf2"),
        ),
    };
    let mut files = vec![
        (target.join("Cargo.toml"), application_toml(name, mode)),
        (target.join("build.rs"), BUILD_RS.to_owned()),
        // What the build writes into the crate, as `cargo new` ignores it:
        // cargo-leptos's site is under `target/`, Trunk's in `dist/`.
        (
            target.join(".gitignore"),
            match mode {
                Mode::Csr => "/target\n/dist\n",
                _ => "/target\n",
            }
            .to_owned(),
        ),
        (target.join("locales/en/main.mf2"), en.to_owned()),
        (target.join("locales/fr/main.mf2"), fr.to_owned()),
    ];
    // The server names the library, which is the package's name.
    let server = || {
        let lib = name.replace('-', "_");
        include_str!("../starters/web/server.rs").replace("hello::", &format!("{lib}::"))
    };
    let sources: Vec<(&str, String)> = match mode {
        Mode::Cli => vec![(
            "src/main.rs",
            include_str!("../starters/cli/main.rs").into(),
        )],
        Mode::Tui => vec![
            (
                "src/main.rs",
                include_str!("../starters/tui/main.rs").into(),
            ),
            ("src/ui.rs", include_str!("../starters/tui/ui.rs").into()),
        ],
        Mode::Ssr => vec![
            ("src/lib.rs", include_str!("../starters/web/ssr.rs").into()),
            ("src/main.rs", server()),
        ],
        Mode::Islands => vec![
            (
                "src/lib.rs",
                include_str!("../starters/web/islands.rs").into(),
            ),
            ("src/main.rs", server()),
        ],
        Mode::Csr => vec![
            ("src/main.rs", include_str!("../starters/web/csr.rs").into()),
            (
                "index.html",
                include_str!("../starters/web/index.html")
                    .replace("data-bin=\"hello\"", &format!("data-bin={name:?}")),
            ),
            (
                "Trunk.toml",
                include_str!("../starters/web/Trunk.toml").into(),
            ),
        ],
    };
    files.extend(
        sources
            .into_iter()
            .map(|(path, body)| (target.join(path), body)),
    );
    files
}

/// A new application's manifest — 19 §1.1's, §1.2's or §1.4's, or the
/// delivery modes' — with the build-override that keeps an edit to a
/// message quick.
fn application_toml(name: &str, mode: Mode) -> String {
    let body = match mode {
        Mode::Cli | Mode::Tui => native_toml(mode),
        Mode::Ssr | Mode::Islands => leptos_toml(name, mode),
        Mode::Csr => csr_toml(mode),
    };
    format!(
        "[package]\n\
         name = {name:?}\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\
         \n\
         {body}\
         \n\
         # The build script compiles the messages again after every edit to\n\
         # them: built optimized, it does so faster.\n\
         {BUILD_OVERRIDE}"
    )
}

fn native_toml(mode: Mode) -> String {
    let mf2 = mode.features_toml();
    let ratatui = if mode == Mode::Tui {
        "ratatui = \"0.30\"\n"
    } else {
        ""
    };
    format!(
        "[dependencies]\n\
         clap = {{ version = \"4\", features = [\"derive\"] }}\n\
         mf2 = {{ version = \"{MAJOR}\", features = [{mf2}] }}\n\
         {ratatui}\
         \n\
         [build-dependencies]\n\
         mf2-build = \"{MAJOR}\"\n"
    )
}

/// cargo-leptos's application: the server natively, the client in wasm.
fn leptos_toml(name: &str, mode: Mode) -> String {
    let leptos = if mode == Mode::Islands {
        ", features = [\"islands\"]"
    } else {
        ""
    };
    let mf2 = mode.features_toml();
    let output = name.replace('-', "_");
    format!(
        "[lib]\n\
         crate-type = [\"cdylib\", \"rlib\"]\n\
         \n\
         [dependencies]\n\
         leptos = {{ version = \"0.9.0-beta\", default-features = false{leptos} }}\n\
         leptos_meta = \"0.9.0-beta\"\n\
         leptos_router = \"0.9.0-beta\"\n\
         mf2 = {{ version = \"{MAJOR}\", features = [{mf2}] }}\n\
         \n\
         axum = {{ version = \"0.8\", optional = true }}\n\
         console_error_panic_hook = {{ version = \"0.1\", optional = true }}\n\
         leptos_axum = {{ version = \"0.9.0-beta\", optional = true }}\n\
         tokio = {{ version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"net\"], optional = true }}\n\
         wasm-bindgen = {{ version = \"0.2\", optional = true }}\n\
         \n\
         [build-dependencies]\n\
         mf2-build = \"{MAJOR}\"\n\
         \n\
         [features]\n\
         hydrate = [\n\
         \x20   \"leptos/hydrate\",\n\
         \x20   \"mf2/hydrate\",\n\
         \x20   \"dep:console_error_panic_hook\",\n\
         \x20   \"dep:wasm-bindgen\",\n\
         ]\n\
         ssr = [\n\
         \x20   \"leptos/ssr\",\n\
         \x20   \"leptos_meta/ssr\",\n\
         \x20   \"leptos_router/ssr\",\n\
         \x20   \"mf2/ssr\",\n\
         \x20   \"mf2/axum\",\n\
         \x20   \"dep:axum\",\n\
         \x20   \"dep:leptos_axum\",\n\
         \x20   \"dep:tokio\",\n\
         ]\n\
         \n\
         [package.metadata.leptos]\n\
         output-name = \"{output}\"\n\
         site-root = \"target/site\"\n\
         site-pkg-dir = \"pkg\"\n\
         site-addr = \"127.0.0.1:3000\"\n\
         reload-port = 3001\n\
         bin-features = [\"ssr\"]\n\
         bin-default-features = false\n\
         lib-features = [\"hydrate\"]\n\
         lib-default-features = false\n\
         lib-profile-release = \"wasm-release\"\n\
         # `cargo leptos watch` watches the crate's sources only.\n\
         watch-additional-files = [\"locales\"]\n\
         \n\
         {WASM_RELEASE}"
    )
}

/// Trunk's application: the client alone, in wasm.
fn csr_toml(mode: Mode) -> String {
    let mf2 = mode.features_toml();
    format!(
        "[dependencies]\n\
         console_error_panic_hook = \"0.1\"\n\
         leptos = {{ version = \"0.9.0-beta\", features = [\"csr\"] }}\n\
         leptos_meta = \"0.9.0-beta\"\n\
         mf2 = {{ version = \"{MAJOR}\", features = [{mf2}] }}\n\
         \n\
         [build-dependencies]\n\
         mf2-build = \"{MAJOR}\"\n\
         \n\
         [profile.release]\n\
         opt-level = \"z\"\n\
         lto = \"fat\"\n\
         codegen-units = 1\n\
         panic = \"abort\"\n\
         strip = true\n"
    )
}

/// The profile a shipped wasm is built with.
const WASM_RELEASE: &str = "\
[profile.wasm-release]
inherits = \"release\"
opt-level = \"z\"
lto = \"fat\"
codegen-units = 1
panic = \"abort\"
strip = true
";

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
        &[
            "add",
            &format!("mf2@{MAJOR}"),
            "-F",
            &mode.features().join(","),
        ],
    )?;
    cargo_add(target, &["add", "--build", &format!("mf2-build@{MAJOR}")])?;
    write_all(&files, args.force)?;
    let next = if mode.web() {
        web_steps(mode)
    } else {
        native_steps(mode)
    };
    println!(
        "\nmf2 init: translations for the crate in {dir}. Next:\n\
         {next}\
         Tip: the build script compiles the messages again after every edit to\n\
         them. Built optimized, it does so faster: add this to the workspace's\n\
         root Cargo.toml:\n\n{BUILD_OVERRIDE}",
        dir = target.display(),
    );
    Ok(())
}

/// Where a call site finds `tr!`.
const TR_STEP: &str = "call `tr!(\"hello\")`: at the root, after the include, it needs no\n\
     \x20    import; any other module writes `use crate::prelude::*;`";

/// What is left to write in a native crate.
fn native_steps(mode: Mode) -> String {
    let ratatui = if mode == Mode::Tui {
        "\x20 4. a message is Ratatui text — `Line::from(tr!(\"hello\"))` — and\n\
         \x20    `mf2::ratatui::set_theme` says how its markup is drawn;\n"
    } else {
        ""
    };
    format!(
        "\x20 1. include the generated module once, at the crate root (src/main.rs\n\
         \x20    or src/lib.rs):\n\
         \x20      mf2::include_generated!();\n\
         \x20 2. at the start of `main`, load the embedded catalogs and take the\n\
         \x20    system's language: `install();`\n\
         \x20 3. {TR_STEP}\n\
         {ratatui}"
    )
}

/// What is left to write in a Leptos crate: what `init` cannot write into
/// its manifest, and the calls.
fn web_steps(mode: Mode) -> String {
    let line = "\x20    (on Leptos 0.8, `mf2`'s feature `leptos-0-8` replaces `leptos`)\n";
    if mode == Mode::Csr {
        return format!(
            "\x20 1. include the generated module once, at the root of the binary\n\
             \x20    (src/main.rs), and in `main` install it before mounting:\n\
             \x20      mf2::include_generated!();\n\
             \x20      install();\n\
             \x20      mf2::leptos::mount_to_body(App);\n\
             {line}\
             \x20 2. {TR_STEP}\n\
             \x20 3. publish the catalogs beside the wasm after each build, with a\n\
             \x20    post_build hook in Trunk.toml that runs\n\
             \x20      mf2 compile --site \"$TRUNK_STAGING_DIR/i18n\"\n\
             \x20    and preload `i18n/index.json` in index.html:\n\
             \x20      <link rel=\"preload\" as=\"fetch\" crossorigin=\"anonymous\" href=\"i18n/index.json\" data-mf2-index />\n"
        );
    }
    let client = if mode == Mode::Islands {
        "\x20      mf2::leptos::hydrate_islands();\n\
         \x20    with `mf2::leptos::islands_gate!();` at the root, and\n\
         \x20    `<IslandsGate/>` first in <body>;\n"
    } else {
        "\x20      mf2::leptos::hydrate_body(App);\n"
    };
    format!(
        "\x20 1. forward the modes to `mf2`, in the crate's [features]:\n\
         \x20      ssr = [..., \"mf2/ssr\", \"mf2/axum\"]\n\
         \x20      hydrate = [..., \"mf2/hydrate\"]\n\
         {line}\
         \x20 2. include the generated module once, at the root of the library\n\
         \x20    (src/lib.rs):\n\
         \x20      mf2::include_generated!();\n\
         \x20 3. on the server, before serving: `<library>::install();`, and on\n\
         \x20    the router\n\
         \x20      .layer(mf2::axum::Negotiator::default())\n\
         \x20      .merge(mf2::axum::catalog_routes())\n\
         \x20 4. in the browser's entry point:\n\
         \x20      install();\n\
         {client}\
         \x20 5. {TR_STEP}\n\
         \x20 6. so that `cargo leptos watch` sees a translation change, add to\n\
         \x20    [package.metadata.leptos]:\n\
         \x20      watch-additional-files = [\"locales\"]\n"
    )
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Mode, starter_files};

    const MODES: [Mode; 5] = [Mode::Cli, Mode::Tui, Mode::Ssr, Mode::Islands, Mode::Csr];

    /// `plan/08` §3.5: the starters have no date in a message and name no
    /// date feature, so a new application links no date code.
    #[test]
    fn the_starters_name_no_date_feature() {
        for mode in MODES {
            for feature in mode.features() {
                assert!(!feature.contains("datetime"), "{}: {feature}", mode.what());
            }
            for (path, body) in starter_files(Path::new("app"), "app", mode) {
                assert!(
                    !body.contains("datetime"),
                    "{}: {} names a date feature",
                    mode.what(),
                    path.display()
                );
                if path.extension().is_some_and(|e| e == "mf2") {
                    for function in [":date", ":time"] {
                        assert!(
                            !body.contains(function),
                            "{}: {} formats a date",
                            mode.what(),
                            path.display()
                        );
                    }
                }
            }
        }
    }
}
