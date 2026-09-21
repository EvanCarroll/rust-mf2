//! Records the compiler version and optimisation level in the binary, so every
//! report says what produced its timings.

use std::process::Command;

fn main() {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |s| s.trim().to_owned());
    let opt_level = std::env::var("OPT_LEVEL").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=PARSER_GATE_RUSTC={version}");
    println!("cargo:rustc-env=PARSER_GATE_OPT_LEVEL={opt_level}");
    println!("cargo:rerun-if-changed=build.rs");
}
