//! A manifest that is well-formed but is not the one this call site was
//! generated against: it is reported, never used.
//!
//! This is the only file that names `__tr_impl!` directly — an application
//! never does. Staleness happens when the generated module and the manifest
//! on disk come from different builds, which is exactly what a long-lived
//! proc-macro server can end up holding.
fn main() {
    let _ = mf2::__tr_impl!(
        bytes b"MF2M\x00\x01\xedo\xeb\xb4\x07\x882\x08\x00\x00" 0x0000_0000_0000_0001u64
        ; ::mf2 ; "plain"
    );
}
