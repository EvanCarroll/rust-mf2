//! Declared after the include.

#[cfg(feature = "after-unqualified")]
pub fn unqualified() -> mf2::Tr {
    tr!("hello")
}

#[cfg(feature = "after-crate-path")]
pub fn crate_path() -> mf2::Tr {
    crate::tr!("hello")
}

#[cfg(feature = "after-use-crate")]
pub fn use_crate() -> mf2::TrArgs {
    use crate::tr;
    tr!("greet", name = "Ada")
}

#[cfg(feature = "after-prelude")]
pub fn prelude() -> mf2::Tr {
    use crate::prelude::*;
    tr!("hello")
}
