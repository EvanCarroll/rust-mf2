//! Resolves only where the translation crate's build script saw `ssr` through
//! `links` (`EMITTED_FOR_SSR`) and `mf2`'s macros kept the server items
//! (`CATALOGS`).

fn main() {
    let emitted: bool = i18n::EMITTED_FOR_SSR;
    let catalogs: usize = i18n::CATALOGS.len();
    let host: &str = i18n::host::HOST;
    println!("{emitted} {catalogs} {host}");
    println!("{}", i18n::report());
}
