//! What the crates this one is built on compiled, held to what this crate's
//! features say for the side being built.
//!
//! Both builds of an application see every feature on its `mf2` line: the
//! browser's build sees the server's date formatter, the server's sees the
//! browser's. A feature therefore has to act on its own side only, and each
//! check here fails the build where one did not — a server's feature that
//! reached the browser, or the reverse. Nothing here exists at run time.

// The catalogs are numbered exactly where this side's date formatter caches:
// natively with ICU4X, in the browser with `host-web-datetime-icu-cached`.
const _: () = assert!(
    mf2_catalog::LOAD_ID
        == cfg!(any(
            all(
                feature = "host-std-datetime-icu",
                not(all(target_arch = "wasm32", target_os = "unknown"))
            ),
            all(
                feature = "host-web-datetime-icu-cached",
                target_arch = "wasm32",
                target_os = "unknown"
            )
        ))
);

// Numbers format through the browser's `Intl` exactly where the browser's
// number formatter is `intl`: with `host-web-number-intl`, and without
// `host-web-number-builtin`, which is stronger. Natively, never.
const _: () = assert!(
    mf2_runtime::INTL_NUMBERS
        == cfg!(all(
            feature = "host-web-number-intl",
            not(feature = "host-web-number-builtin"),
            target_arch = "wasm32",
            target_os = "unknown"
        ))
);

// The data ICU4X formats dates from. In the browser: the catalog's blob, its
// formatter built for each placeholder, whatever the server's features are.
#[cfg(all(
    feature = "host-web-datetime-icu",
    not(feature = "host-web-datetime-icu-cached"),
    target_arch = "wasm32",
    target_os = "unknown"
))]
const _: fn(mf2_fn_datetime::icu::DefaultData) -> mf2_fn_datetime::icu::Blob = |data| data;
// In the browser, with the cache it asked for.
#[cfg(all(
    feature = "host-web-datetime-icu-cached",
    target_arch = "wasm32",
    target_os = "unknown"
))]
const _: fn(mf2_fn_datetime::icu::DefaultData) -> mf2_fn_datetime::icu::CachedBlob = |data| data;
// Natively: always with the cache, which costs no download there.
#[cfg(all(
    feature = "host-std-datetime-icu",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
const _: fn(mf2_fn_datetime::icu::DefaultData) -> mf2_fn_datetime::icu::CachedBlob = |data| data;
