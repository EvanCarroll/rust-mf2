//! `mf2`'s own test over a generated module, through a dev-dependency that
//! depends on `mf2` again.

#[test]
fn fixture_follows_this_build() {
    let line = fixture::report();
    println!("{line}");
    assert_eq!(fixture::BUILD_SAW_SOURCE, "links");
    assert_eq!(fixture::MACRO_SAW_SSR, cfg!(feature = "ssr"));
}
