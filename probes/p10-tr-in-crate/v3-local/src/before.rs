//! Declared before the include.

#[cfg(feature = "before-unqualified")]
pub fn unqualified() -> mf2::Tr {
    tr!("hello")
}

#[cfg(feature = "before-crate-path")]
pub fn crate_path() -> mf2::Tr {
    crate::tr!("hello")
}

#[cfg(feature = "before-use-crate")]
pub fn use_crate() -> mf2::TrArgs {
    use crate::tr;
    tr!("greet", name = "Ada")
}

#[cfg(feature = "before-prelude")]
pub fn prelude() -> mf2::Tr {
    use crate::prelude::*;
    tr!("hello")
}
