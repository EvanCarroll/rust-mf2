//! `cargo xtask islands-zero`: a server-only component costs the client
//! **zero** bytes.
//!
//! `examples/demo-islands` is built for the client twice — as it is, and with
//! `more-server`, which adds one more server-only component holding a call
//! site in every position (text, attribute, argument, markup) — each by
//! the size method (`wasm32-unknown-unknown` / `wasm-release` /
//! `wasm-bindgen` / `wasm-opt -Oz` / `brotli -q 11`).
//!
//! **What "zero" means, measured.** The first run (2026-09-23) found the two
//! shipped files 1 byte apart raw and equal compressed, with the same functions
//! and the same data — in a different *order*: a string constant the
//! server-only component shares with an island ("card") moves where the
//! linker merges it, and the padding between constants moves with it. So
//! the gate is on what a component could actually add, not on layout:
//!
//! * the **code** section is the same size, with the same function count;
//! * the **data** section differs by at most [`PADDING`] bytes;
//! * the shipped file, raw and brotli, by at most the same.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::browser_app_size::{self, Sizes};
use crate::cmd;
use crate::error::{Error, Result};

const LIB: &str = "demo_islands";

/// What reordering the data section may cost in alignment padding, in bytes.
const PADDING: u64 = 16;

/// The wasm section ids the gate reads.
const CODE: u8 = 10;
const DATA: u8 = 11;

pub(crate) fn run(root: &Path) -> Result<()> {
    let app = root.join("examples").join("demo-islands");
    let out = root.join("target").join("islands-zero");
    let without = build(root, &app, &out, "hydrate", "without")?;
    let with = build(root, &app, &out, "hydrate,more-server", "with")?;
    let shipped = |name: &str| out.join(name).join("opt.wasm");
    let (bytes_without, bytes_with) = (read(&shipped("without"))?, read(&shipped("with"))?);
    let (sections_without, sections_with) = (sections(&bytes_without), sections(&bytes_with));
    let section = |all: &[(u8, u64, u64)], id: u8| {
        all.iter()
            .find(|(i, _, _)| *i == id)
            .map_or((0, 0), |(_, size, count)| (*size, *count))
    };
    let (code_without, functions_without) = section(&sections_without, CODE);
    let (code_with, functions_with) = section(&sections_with, CODE);
    let (data_without, _) = section(&sections_without, DATA);
    let (data_with, _) = section(&sections_with, DATA);

    println!("# A server-only component's client cost\n");
    println!("| Build | code section | functions | data section | shipped, raw | shipped, br |");
    println!("|---|---:|---:|---:|---:|---:|");
    println!(
        "| demo-islands | {code_without} | {functions_without} | {data_without} | {} | {} |",
        without.opt_raw, without.opt_br
    );
    println!(
        "| + more-server | {code_with} | {functions_with} | {data_with} | {} | {} |",
        with.opt_raw, with.opt_br
    );
    println!(
        "\nDifference: code {} B, data {} B, shipped {} B raw / {} B br; the files are {}.",
        signed(code_with, code_without),
        signed(data_with, data_without),
        signed(with.opt_raw, without.opt_raw),
        signed(with.opt_br, without.opt_br),
        if bytes_with == bytes_without {
            "byte-identical"
        } else {
            "not byte-identical"
        },
    );

    let failed = if code_with != code_without || functions_with != functions_without {
        Some("a server-only component changed the client's code")
    } else if data_with.abs_diff(data_without) > PADDING {
        Some("a server-only component changed the client's data by more than padding")
    } else if with.opt_raw.abs_diff(without.opt_raw) > PADDING
        || with.opt_br.abs_diff(without.opt_br) > PADDING
    {
        Some("a server-only component changed the shipped client by more than padding")
    } else {
        None
    };
    if let Some(status) = failed {
        return Err(Error::CommandFailed {
            command: "islands-zero".to_owned(),
            status: status.to_owned(),
            stderr: String::new(),
        });
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

/// Each section of a wasm module: its id, its size in bytes, and the item
/// count its body starts with (for the code section, the function count).
fn sections(module: &[u8]) -> Vec<(u8, u64, u64)> {
    let mut out = Vec::new();
    // Past the magic number and the version.
    let mut at = 8;
    while let Some(&id) = module.get(at) {
        let Some((size, used)) = leb(module.get(at + 1..).unwrap_or_default()) else {
            break;
        };
        let body = at + 1 + used;
        let count = leb(module.get(body..).unwrap_or_default()).map_or(0, |(n, _)| n);
        out.push((id, size, count));
        at = body.saturating_add(usize::try_from(size).unwrap_or(usize::MAX));
    }
    out
}

/// An unsigned LEB128 number and how many bytes it took.
fn leb(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (i, byte) in bytes.iter().take(10).enumerate() {
        value |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

fn signed(a: u64, b: u64) -> String {
    if a >= b {
        format!("+{}", a - b)
    } else {
        format!("-{}", b - a)
    }
}

/// One client build of the example with `features`, measured.
fn build(root: &Path, app: &Path, out: &Path, features: &str, name: &str) -> Result<Sizes> {
    let target = out.join("target");
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("--quiet"),
        OsStr::new("--lib"),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
        OsStr::new("--target"),
        OsStr::new("wasm32-unknown-unknown"),
        OsStr::new("--profile"),
        OsStr::new("wasm-release"),
    ];
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    cmd::run_inherit_env(
        &cmd::cargo(),
        &args,
        app,
        &[
            ("CARGO_TARGET_DIR", target.as_os_str()),
            ("CARGO_BUILD_JOBS", jobs.as_os_str()),
        ],
    )?;
    let wasm = target
        .join("wasm32-unknown-unknown")
        .join("wasm-release")
        .join(format!("{LIB}.wasm"));
    browser_app_size::ship(root, &wasm, &out.join(name), LIB)
}
