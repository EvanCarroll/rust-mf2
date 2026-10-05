//! Carries this crate's features to the build script of the crate that
//! includes the generated module.
//!
//! `links = "mf2-v3"` makes cargo hand each `cargo::metadata=KEY=VALUE` below
//! to the build script of every crate that names `mf2` as a dependency, as
//! `DEP_MF2_V3_KEY`. `mf2_build::run()` reads them to decide what to emit, so
//! that no translation crate declares or forwards a feature of its own.

fn main() {
    // Sorted, comma-separated, `default` left out: cargo spells `leptos-0-8`
    // as `CARGO_FEATURE_LEPTOS_0_8`, and no feature of this crate has a `_`.
    let mut features: Vec<String> = std::env::vars_os()
        .filter_map(|(key, _)| {
            key.to_str()?
                .strip_prefix("CARGO_FEATURE_")
                .map(|f| f.to_ascii_lowercase().replace('_', "-"))
        })
        .filter(|f| f != "default")
        .collect();
    features.sort();
    println!("cargo::metadata=features={}", features.join(","));
    println!(
        "cargo::metadata=target={}",
        std::env::var("TARGET").unwrap_or_default()
    );
    println!("cargo::metadata=version={}", env!("CARGO_PKG_VERSION"));
    // `mf2_date_rewrite`: whether the browser can rewrite a hydrated page's
    // dates because the server formats them with another formatter
    // (`plan/08` §4.3). Both builds of an application see both sides'
    // features, so where the pair is known and never rewrites (the same
    // formatter on both sides, or ICU4X on the server under `Intl`), the
    // client links no rewrite code and the page states no formatter. A side
    // whose formatter this build cannot see may rewrite: the page decides.
    println!("cargo::rustc-check-cfg=cfg(mf2_date_rewrite)");
    let on = |feature: &str| std::env::var_os(format!("CARGO_FEATURE_{feature}")).is_some();
    // The strongest of a side's features formats, as `leptos::links::date_formatter`.
    let browser = if on("HOST_WEB_DATETIME_ICU") {
        Some("icu")
    } else if on("HOST_WEB_DATETIME_INTL") {
        Some("intl")
    } else if on("HOST_WEB_DATETIME_ISO") {
        Some("iso")
    } else {
        None
    };
    let server = if on("HOST_STD_DATETIME_ICU") {
        Some("icu")
    } else if on("HOST_STD_DATETIME_ISO") {
        Some("iso")
    } else {
        None
    };
    let rewrites = match (server, browser) {
        (Some(server), Some(browser)) => server != browser && (server, browser) != ("icu", "intl"),
        _ => true,
    };
    if rewrites {
        println!("cargo::rustc-cfg=mf2_date_rewrite");
    }
    // The features are the unit's own: a change makes another unit, which
    // runs this again. Nothing else here can change the output.
    println!("cargo::rerun-if-changed=build.rs");
}
