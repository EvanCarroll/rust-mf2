//! The command tree 1.x promises (`docs/versioning.md`): every command and
//! its arguments as clap declares them, held against `api.txt` beside the
//! manifest. `cargo xtask api` rewrites the file (`MF2_CLI_API_WRITE=1`);
//! otherwise the test fails on any difference, so a changed flag is
//! committed together with its listing.

use std::fmt::Write as _;

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::Cli;

const HEADER: &str = "# The public API that 1.x promises (docs/versioning.md). Written by \
                      `cargo xtask api`, checked by `cargo xtask ci`; commit it with the change.\n\
                      # The `mf2` command tree: commands, then each argument, its value and \
                      the values it accepts.\n";

/// The listing of `mf2`'s command tree.
fn render() -> String {
    let mut out = String::from(HEADER);
    // Built, so that positional arguments have their indices.
    let mut cli = Cli::command();
    cli.build();
    command(&mut out, &cli, "mf2");
    out
}

fn command(out: &mut String, cmd: &Command, path: &str) {
    let _ = writeln!(out, "{path}");
    // `--help` and `--version` are clap's, the same on every command.
    let mut args: Vec<&Arg> = cmd
        .get_arguments()
        .filter(|a| !matches!(a.get_action(), ArgAction::Help | ArgAction::Version))
        .collect();
    args.sort_by_key(|a| a.get_id().as_str());
    for arg in args {
        let _ = writeln!(out, "  {}", describe(arg));
    }
    let mut subs: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .collect();
    subs.sort_by_key(|c| c.get_name());
    for sub in subs {
        command(out, sub, &format!("{path} {}", sub.get_name()));
    }
}

fn describe(arg: &Arg) -> String {
    let mut s = match (arg.get_long(), arg.get_short()) {
        (Some(long), Some(short)) => format!("--{long}, -{short}"),
        (Some(long), None) => format!("--{long}"),
        (None, Some(short)) => format!("-{short}"),
        (None, None) => format!(
            "<{}> (position {})",
            arg.get_id(),
            arg.get_index().unwrap_or_default()
        ),
    };
    let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
    if takes_value && !arg.is_positional() {
        let names: Vec<String> = arg.get_value_names().map_or_else(
            || vec![format!("<{}>", arg.get_id().as_str().to_uppercase())],
            |v| v.iter().map(|n| format!("<{n}>")).collect(),
        );
        s.push(' ');
        s.push_str(&names.join(" "));
    }
    let flag = matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse);
    let values: Vec<String> = (if flag {
        Vec::new()
    } else {
        arg.get_possible_values()
    })
    .iter()
    .filter(|v| !v.is_hide_set())
    .map(|v| v.get_name().to_owned())
    .collect();
    if !values.is_empty() {
        let _ = write!(s, " [{}]", values.join("|"));
    }
    if matches!(arg.get_action(), ArgAction::Append) {
        s.push_str(" (repeatable)");
    }
    if arg.is_required_set() {
        s.push_str(" (required)");
    }
    if arg.is_global_set() {
        s.push_str(" (global)");
    }
    s
}

#[test]
fn api_txt() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("api.txt");
    let built = render();
    if std::env::var_os("MF2_CLI_API_WRITE").is_some() {
        std::fs::write(&path, &built).expect("write api.txt");
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == built,
        "crates/mf2-cli/api.txt differs from the command tree; run `cargo xtask api` and \
         commit it with the change.\n--- committed\n{committed}\n--- built\n{built}"
    );
}
