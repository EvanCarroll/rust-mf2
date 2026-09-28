//! Declared after `include_generated!`.

#[cfg(feature = "after-unqualified")]
pub fn unqualified() -> mf2::Tr {
    tr!("hello")
}

#[cfg(feature = "after-crate-path")]
pub fn crate_path() -> mf2::Tr {
    crate::tr!("hello")
}

#[cfg(feature = "after-use-crate")]
pub fn use_crate() -> mf2::Tr {
    use crate::tr;
    tr!("hello")
}

#[cfg(feature = "after-super-path")]
pub fn super_path() -> mf2::Tr {
    super::tr!("hello")
}
