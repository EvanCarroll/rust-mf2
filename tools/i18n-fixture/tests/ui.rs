//! The `tr!` compile-fail set.
//!
//! It lives in the fixture because the fixture is an i18n crate exactly as
//! an application writes one: these files are compiled against *its*
//! generated `tr!`, its manifest and its corpus, which is the only way the
//! macro's checks can be exercised the way an application meets them.
//!
//! `TRYBUILD=overwrite cargo test -p mf2-i18n-fixture --test ui` rewrites the
//! expected output; every `.stderr` here was read before it was committed —
//! the message *is* the feature.

#[test]
fn a_call_site_the_manifest_refuses_does_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
