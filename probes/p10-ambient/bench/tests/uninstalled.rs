//! A native-only build that formats with nothing installed panics, and the
//! message names `install()` (plans/18, "Decided without asking").

use probe_i18n::tr;

#[test]
#[should_panic(expected = "call install() at start-up")]
fn formatting_before_install_panics_naming_install() {
    let _ = ambient::to_string::<false>(&tr!("app.title"));
}
