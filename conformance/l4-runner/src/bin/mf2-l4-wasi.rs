//! `mf2-l4-wasi`: conformance layer L4 on `wasm32-wasip1` (under wasmtime).
//! Reads a bundle of compiled suite cases on stdin (`encode_cases`) and
//! writes one canonical record per case on stdout — the same code the native
//! harness runs, so the two outputs must be identical byte for byte.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("mf2-l4-wasi: reading stdin: {e}");
        return ExitCode::FAILURE;
    }
    let cases = match mf2_l4_runner::decode_cases(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mf2-l4-wasi: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for case in &cases {
        let line = match mf2_l4_runner::run(case) {
            Ok(r) => r.line(),
            Err(e) => format!("ERROR {e}"),
        };
        if writeln!(out, "{}\t{line}", case.id).is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
