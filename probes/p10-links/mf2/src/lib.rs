//! A stand-in for `mf2` (Phase 10 A2 probe; `plans/18-phase-10-work-order.md`).
//!
//! Two ways for a generated module, compiled in *another* crate, to follow
//! the features this crate was built with:
//!
//! * the build script of the including crate reads them from `links`
//!   metadata (`DEP_MF2_V2_FEATURES`, printed by this crate's `build.rs`) and
//!   writes different code;
//! * the generated code stays the same and wraps each choice in one of the
//!   cfg-forwarding macros below, which this crate defines once per value of
//!   its own `cfg(feature = …)`. The choice is then made by how *this* crate
//!   was compiled, whatever the including crate declares.

/// Includes the module the build script wrote to `OUT_DIR`, as the real
/// `mf2::include_generated!()` does.
#[macro_export]
macro_rules! include_generated {
    () => {
        include!(concat!(env!("OUT_DIR"), "/mf2_generated.rs"));
    };
}

/// The features this crate was compiled with, read with `cfg!` at compile
/// time: the ground truth the build scripts' view is compared with.
#[must_use]
pub fn compiled_features() -> String {
    let all = [
        ("ssr", cfg!(feature = "ssr")),
        ("hydrate", cfg!(feature = "hydrate")),
        ("csr", cfg!(feature = "csr")),
        ("leptos", cfg!(feature = "leptos")),
        ("axum", cfg!(feature = "axum")),
        ("native", cfg!(feature = "native")),
        ("ratatui", cfg!(feature = "ratatui")),
        ("fn-number", cfg!(feature = "fn-number")),
        ("fn-datetime", cfg!(feature = "fn-datetime")),
        ("datetime-icu", cfg!(feature = "datetime-icu")),
        ("datetime-intl", cfg!(feature = "datetime-intl")),
        ("intl", cfg!(feature = "intl")),
        ("static-locale", cfg!(feature = "static-locale")),
        ("mark-fallback-lang", cfg!(feature = "mark-fallback-lang")),
        ("compile", cfg!(feature = "compile")),
    ];
    let on: Vec<&str> = all.iter().filter(|(_, on)| *on).map(|(n, _)| *n).collect();
    on.join(",")
}

/// Stand-ins for the hosts the generated `host` module picks between.
pub mod host_std {
    /// The native host.
    pub static HOST: &str = "host_std::HOST";
}

/// Stand-ins for the browser hosts.
pub mod host_web {
    /// The browser host without dates.
    pub static HOST: &str = "host_web::HOST";
    /// With `datetime-intl`.
    pub static INTL_HOST: &str = "host_web::INTL_HOST";
    /// With `datetime-icu`.
    pub static ZONES_HOST: &str = "host_web::ZONES_HOST";
}

// The cfg-forwarding macros. Each is defined twice, under a `cfg` and its
// negation, so exactly one definition exists in any build: the tokens pass
// through, or vanish.

/// Keeps its input when this crate has `ssr`.
#[cfg(feature = "ssr")]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_ssr {
    ($($t:tt)*) => { $($t)* };
}
/// Drops its input: this crate has no `ssr`.
#[cfg(not(feature = "ssr"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_ssr {
    ($($t:tt)*) => {};
}

/// Keeps its input when this crate has no `ssr`.
#[cfg(not(feature = "ssr"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_not_ssr {
    ($($t:tt)*) => { $($t)* };
}
/// Drops its input: this crate has `ssr`.
#[cfg(feature = "ssr")]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_not_ssr {
    ($($t:tt)*) => {};
}

/// Keeps its input when this crate has `hydrate`.
#[cfg(feature = "hydrate")]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_hydrate {
    ($($t:tt)*) => { $($t)* };
}
/// Drops its input: no `hydrate`.
#[cfg(not(feature = "hydrate"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_hydrate {
    ($($t:tt)*) => {};
}

/// The browser host this crate's date features ask for, as one item: the
/// three-way choice the generated `host` module makes today with
/// `#[cfg(all(not(feature = "ssr"), …))]` on the *including* crate's
/// features. `$name` is the alias the generated code wants.
#[cfg(feature = "ssr")]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($name:ident) => {
        pub use $crate::host_std::HOST as $name;
    };
}
#[cfg(all(not(feature = "ssr"), feature = "datetime-intl"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($name:ident) => {
        pub use $crate::host_web::INTL_HOST as $name;
    };
}
#[cfg(all(
    not(feature = "ssr"),
    not(feature = "datetime-intl"),
    feature = "datetime-icu"
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($name:ident) => {
        pub use $crate::host_web::ZONES_HOST as $name;
    };
}
#[cfg(all(
    not(feature = "ssr"),
    not(feature = "datetime-intl"),
    not(feature = "datetime-icu")
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($name:ident) => {
        pub use $crate::host_web::HOST as $name;
    };
}
