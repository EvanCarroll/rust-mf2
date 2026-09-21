//! Records the compiler version, the optimisation level and the versions of
//! the compressors in the binary, so every report says what produced its
//! numbers (compressed sizes depend on the compressor's version).

use std::path::Path;
use std::process::Command;

/// Crates whose resolved version the size report prints.
const TRACKED: [&str; 4] = ["brotli", "flate2", "miniz_oxide", "zlib-rs"];

/// `name = "x"` followed by `version = "y"` in the workspace `Cargo.lock`.
fn locked_version(lock: &str, name: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == format!("name = \"{name}\"") {
            let v = lines.next()?.trim().strip_prefix("version = \"")?;
            return v.strip_suffix('"').map(str::to_owned);
        }
    }
    None
}

fn main() {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |s| s.trim().to_owned());
    let opt_level = std::env::var("OPT_LEVEL").unwrap_or_else(|_| "unknown".to_owned());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=CATALOG_BENCH_RUSTC={version}");
    println!("cargo:rustc-env=CATALOG_BENCH_OPT_LEVEL={opt_level}");
    println!("cargo:rustc-env=CATALOG_BENCH_PROFILE={profile}");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let lock_path = Path::new(&manifest_dir).join("../../Cargo.lock");
    let lock = std::fs::read_to_string(&lock_path).unwrap_or_default();
    let versions: Vec<String> = TRACKED
        .iter()
        .map(|name| {
            let v = locked_version(&lock, name).unwrap_or_else(|| "?".to_owned());
            format!("{name} {v}")
        })
        .collect();
    println!(
        "cargo:rustc-env=CATALOG_BENCH_CRATES={}",
        versions.join(", ")
    );
    println!("cargo:rerun-if-changed={}", lock_path.display());
    println!("cargo:rerun-if-changed=build.rs");
}
