//! The P0.4 threshold as a test: 100 % of CLDR samples, every locale, both kinds.

use plural_rules::check;
use plural_rules::cldr::{self, default_dir};

#[test]
fn every_cldr_sample_passes() {
    let cldr = cldr::load(&default_dir()).expect("load CLDR plural rules");
    assert_eq!(cldr.version, "48");
    let r = check::run(&cldr);
    for f in &r.failures {
        eprintln!("FAIL {} {} {} {:?}: {}", f.kind.name(), f.locale, f.category.as_str(), f.sample, f.cause);
    }
    assert!(r.failures.is_empty(), "{} failures", r.failures.len());
    assert!(r.unsampled.is_empty());
    assert_eq!(r.cardinal.locales, 224);
    assert_eq!(r.ordinal.locales, 108);
    assert!(r.cardinal.samples() > 10_000);
}
