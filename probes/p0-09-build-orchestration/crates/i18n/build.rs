//! Parses locales/, writes the manifest + catalogs to OUT_DIR and generates
//! the module included by src/lib.rs (plans/05-tooling.md §4).
fn main() {
    p09_build::Build::new()
        .source_locale("en")
        .facade("p09_mf2")
        .run_or_exit();
}
