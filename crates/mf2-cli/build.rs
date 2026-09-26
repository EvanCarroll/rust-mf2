//! `cfg(mf2_workspace)`: this crate is being built in its repository, not
//! from its package. The unit tests that read the repository around the
//! crate (`src/workspace_tests.rs`, `src/convert/leptos_fluent/tests.rs`)
//! are left out of the package, and compile only where they are
//! (`plans/17-phase-9-work-order.md` A4).

fn main() {
    println!("cargo::rustc-check-cfg=cfg(mf2_workspace)");
    println!("cargo::rerun-if-changed=src");
    if std::path::Path::new("src/workspace_tests.rs").is_file() {
        println!("cargo::rustc-cfg=mf2_workspace");
    }
}
