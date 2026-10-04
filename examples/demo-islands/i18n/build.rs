//! The whole build script: parse `locales/`, write the manifest and the
//! catalogs into `OUT_DIR`, and generate the module `src/lib.rs` includes,
//! for the features `mf2` has in this build.

fn main() {
    mf2_build::run();
}
