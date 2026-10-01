//! Running child processes.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// The cargo that invoked us (`$CARGO`), so child builds use the same toolchain.
pub(crate) fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn describe(program: &OsStr, args: &[&OsStr]) -> String {
    let mut s = program.to_string_lossy().into_owned();
    for a in args {
        s.push(' ');
        s.push_str(&a.to_string_lossy());
    }
    s
}

/// Runs a command with inherited stdio in `dir`; fails on a non-zero exit.
pub(crate) fn run_inherit(program: &OsStr, args: &[&OsStr], dir: &Path) -> Result<()> {
    run_inherit_env(program, args, dir, &[])
}

/// [`run_inherit`], with environment variables set for the child.
pub(crate) fn run_inherit_env(
    program: &OsStr,
    args: &[&OsStr],
    dir: &Path,
    envs: &[(&str, &OsStr)],
) -> Result<()> {
    let shown = describe(program, args);
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .envs(envs.iter().map(|(k, v)| (*k, *v)))
        .status()
        .map_err(|source| Error::Spawn {
            program: shown.clone(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            command: shown,
            status: status.to_string(),
            stderr: String::new(),
        })
    }
}

/// `rustup toolchain install TOOLCHAIN --profile minimal --target TARGET`,
/// skipped when rustup already has both. The install checks the network for
/// updates even when nothing is missing, so it failed every time this
/// machine lost its connection, though the toolchain was there.
pub(crate) fn rustup_install(dir: &Path, toolchain: &str, target: &str) -> Result<()> {
    let rustup = OsStr::new("rustup");
    let list = ["target", "list", "--installed", "--toolchain", toolchain].map(OsStr::new);
    let present = run_capture(rustup, &list, dir, &[]).is_ok_and(|out| {
        String::from_utf8_lossy(&out)
            .lines()
            .any(|l| l.trim() == target)
    });
    if present {
        return Ok(());
    }
    let install = [
        "toolchain",
        "install",
        toolchain,
        "--profile",
        "minimal",
        "--target",
        target,
    ]
    .map(OsStr::new);
    run_capture(rustup, &install, dir, &[]).map(|_| ())
}

/// Runs a command in `dir`, capturing stdout; stderr is captured and reported on failure.
pub(crate) fn run_capture(
    program: &OsStr,
    args: &[&OsStr],
    dir: &Path,
    envs: &[(&str, &str)],
) -> Result<Vec<u8>> {
    let shown = describe(program, args);
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .output()
        .map_err(|source| Error::Spawn {
            program: shown.clone(),
            source,
        })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        Err(Error::CommandFailed {
            command: shown,
            status: output.status.to_string(),
            stderr: if stderr.is_empty() {
                String::new()
            } else {
                format!(":\n{stderr}")
            },
        })
    }
}
