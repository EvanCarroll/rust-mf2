//! `cargo xtask l4-wasi`: conformance layer L4 on `wasm32-wasip1`
//! (plans/01-conformance.md §3; plans/10-phase-3-work-order.md A9). Every
//! L4 case of the suite — each test that is not a syntax or data-model
//! error, compiled natively for its locale, unstripped and stripped, in the
//! all-features and in the default configuration (L4d) — is formatted by
//! `mf2-l4-runner` natively and by its `mf2-l4-wasi` binary under wasmtime;
//! the two outputs must be identical byte for byte.
//!
//! `--generated N` adds N generated cases (plans/10-phase-3-work-order.md
//! A10; `mf2_conformance::l4gen`), unstripped and stripped: the seeds of
//! `generated_l4`'s nightly million, evenly spaced — the sampled
//! native = wasip1 run over generated input.
//!
//! wasmtime is pinned ([`WASMTIME`]) and installed into `target/tools` by
//! cargo (`cargo install --root target/tools wasmtime-cli --version …
//! --locked`), never globally.

use std::ffi::OsStr;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use mf2_conformance::abnf::Grammar;
use mf2_conformance::spec::{ABNF, spec_path};
use mf2_conformance::{SUITE_DIR, Suite, TestKind, l4gen};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The pinned wasmtime release.
pub(crate) const WASMTIME: &str = "49.0.0";

/// `generated_l4`'s default seed and its nightly case count.
const GEN_SEED: u64 = 0x6d66_3274_776f;
const GEN_NIGHTLY: u64 = 1_000_000;

pub(crate) fn run(root: &Path, generated: Option<u64>) -> Result<()> {
    let wasmtime = root.join("target/tools/bin/wasmtime");
    let version = cmd::run_capture(wasmtime.as_os_str(), &[OsStr::new("--version")], root, &[])
        .map_err(|_| Error::WasmtimeMissing(WASMTIME))?;
    let version = String::from_utf8_lossy(&version);
    if !version.starts_with(&format!("wasmtime {WASMTIME}")) {
        return Err(Error::WasmtimeVersion {
            want: WASMTIME,
            got: version.trim().to_owned(),
        });
    }

    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let mut cases = Vec::new();
    for test in suite.tests() {
        if test.kind != TestKind::Other {
            continue;
        }
        let (mut unstripped, mut stripped) = mf2_conformance::l4::cases(test).map_err(Error::L4)?;
        unstripped.id.push_str("/unstripped");
        stripped.id.push_str("/stripped");
        // The default configuration too (layer L4d): the core's neutral
        // handlers, which the all-features registry no longer uses.
        for c in [&unstripped, &stripped] {
            let mut d = c.clone();
            d.id.push_str("/default");
            d.config = mf2_l4_runner::Config::Default;
            cases.push(d);
        }
        cases.push(unstripped);
        cases.push(stripped);
    }
    let suite_cases = cases.len();
    // The locale-output goldens (plans/11 A8): the Rust backends' output,
    // identical on both targets.
    for family in mf2_conformance::goldens::FAMILIES {
        for g in mf2_conformance::goldens::cases(family).map_err(Error::L4)? {
            cases.push(g.case);
        }
    }
    let golden_cases = cases.len() - suite_cases;
    if let Some(n) = generated.filter(|&n| n > 0) {
        let text = fsx::read_to_string(&spec_path(root, ABNF))?;
        let grammar = Grammar::parse(&text).map_err(|e| Error::L4(e.to_string()))?;
        let step = (GEN_NIGHTLY / n).max(1);
        for i in 0..n {
            let g = l4gen::case(&grammar, GEN_SEED.wrapping_add(i * step)).map_err(Error::L4)?;
            cases.push(g.unstripped);
            cases.push(g.stripped);
        }
    }
    let native: Vec<String> = cases
        .iter()
        .map(|c| {
            let line = match mf2_l4_runner::run(c) {
                Ok(r) => r.line(),
                Err(e) => format!("ERROR {e}"),
            };
            format!("{}\t{line}", c.id)
        })
        .collect();

    let dir = root.join("target/l4-wasi");
    let bundle = mf2_l4_runner::encode_cases(&cases);
    fsx::write(&dir.join("cases.bin"), &bundle)?;
    eprintln!(
        "l4-wasi: {} cases; building mf2-l4-wasi for wasm32-wasip1",
        cases.len()
    );
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "-p",
            "mf2-l4-runner",
            "--bin",
            "mf2-l4-wasi",
        ]
        .map(OsStr::new),
        root,
    )?;
    let wasm = root.join("target/wasm32-wasip1/release/mf2-l4-wasi.wasm");
    eprintln!("l4-wasi: running under {}", version.trim());
    let mut child = Command::new(&wasmtime)
        .arg("run")
        .arg(&wasm)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|source| Error::Spawn {
            program: wasmtime.display().to_string(),
            source,
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&bundle)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(Error::CommandFailed {
            command: format!("wasmtime run {}", wasm.display()),
            status: output.status.to_string(),
            stderr: String::new(),
        });
    }
    let wasi_text =
        String::from_utf8(output.stdout).map_err(|e| Error::Utf8("mf2-l4-wasi".to_owned(), e))?;
    let wasi: Vec<&str> = wasi_text.lines().collect();
    fsx::write(
        &dir.join("native.txt"),
        (native.join("\n") + "\n").as_bytes(),
    )?;
    fsx::write(&dir.join("wasi.txt"), wasi_text.as_bytes())?;

    let mut differ = 0;
    for (i, n) in native.iter().enumerate() {
        match wasi.get(i) {
            Some(w) if *w == n => {}
            Some(w) => {
                differ += 1;
                if differ <= 10 {
                    eprintln!("l4-wasi: differs\n  native {n}\n  wasi   {w}");
                }
            }
            None => differ += 1,
        }
    }
    if wasi.len() != native.len() {
        eprintln!(
            "l4-wasi: {} native records, {} from wasm",
            native.len(),
            wasi.len()
        );
    }
    let errors = native.iter().filter(|l| l.contains("\tERROR ")).count();
    if differ > 0 || wasi.len() != native.len() || errors > 0 {
        return Err(Error::L4(format!(
            "{differ} of {} records differ between native and wasm32-wasip1; {errors} runner errors \
             (target/l4-wasi/native.txt, wasi.txt)",
            native.len()
        )));
    }
    eprintln!(
        "l4-wasi: {} records identical on native and wasm32-wasip1 ({} suite tests, unstripped and \
         stripped, in both configurations; {golden_cases} golden cases; {} generated cases, \
         unstripped and stripped)",
        native.len(),
        suite_cases / 4,
        (native.len() - suite_cases - golden_cases) / 2
    );
    Ok(())
}
