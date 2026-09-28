//! Variant 1 (today's form) and variant 2 (the lint allowed).
#![cfg_attr(
    feature = "allow-52234",
    allow(macro_expanded_macro_exports_accessed_by_absolute_paths)
)]

pub mod before;

#[cfg(not(feature = "path-mod"))]
mf2::include_generated!();

#[cfg(feature = "path-mod")]
#[path = concat!(env!("OUT_DIR"), "/mf2_generated.rs")]
pub mod generated;

pub mod after;

#[cfg(feature = "root-unqualified")]
pub fn root_unqualified() -> mf2::Tr {
    tr!("hello")
}

#[cfg(feature = "root-crate-path")]
pub fn root_crate_path() -> mf2::Tr {
    crate::tr!("hello")
}

#[cfg(feature = "root-self-path")]
pub fn root_self_path() -> mf2::Tr {
    self::tr!("hello")
}

// A macro of the crate's own that names `tr!` through `$crate`.
#[cfg(feature = "dollar-crate")]
macro_rules! hello {
    () => {
        $crate::tr!("hello")
    };
}

#[cfg(feature = "dollar-crate")]
pub fn dollar_crate() -> mf2::Tr {
    hello!()
}
