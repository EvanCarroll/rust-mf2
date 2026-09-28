//! Declared before `include_generated!`.

#[cfg(feature = "before-unqualified")]
pub fn unqualified() -> mf2::Tr {
    tr!("hello")
}

#[cfg(feature = "before-crate-path")]
pub fn crate_path() -> mf2::Tr {
    crate::tr!("hello")
}

#[cfg(feature = "before-use-crate")]
pub fn use_crate() -> mf2::Tr {
    use crate::tr;
    tr!("hello")
}

#[cfg(feature = "before-super-path")]
pub fn super_path() -> mf2::Tr {
    super::tr!("hello")
}
