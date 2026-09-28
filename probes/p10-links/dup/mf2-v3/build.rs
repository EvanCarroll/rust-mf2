//! Carries this build's feature set to the build scripts of the crates that
//! depend on this one (`links = "mf2-v2"` → `DEP_MF2_V2_FEATURES`).

use std::env;

fn main() {
    let mut features: Vec<String> = env::vars()
        .filter_map(|(key, _)| {
            key.strip_prefix("CARGO_FEATURE_")
                .map(|f| f.to_ascii_lowercase().replace('_', "-"))
        })
        .filter(|f| f != "default")
        .collect();
    features.sort();
    println!("cargo::metadata=features={}", features.join(","));
    // For the host/target row: which instance a dependent was handed.
    println!(
        "cargo::metadata=target={}",
        env::var("TARGET").unwrap_or_default()
    );
    // Reads nothing else: a feature change is a new unit anyway.
    println!("cargo::rerun-if-changed=build.rs");
}
