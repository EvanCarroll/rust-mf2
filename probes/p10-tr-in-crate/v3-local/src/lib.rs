//! Variant 3: a path-addressable in-crate `tr!`.

pub mod before;

mf2::include_generated!("mf2_generated_v3.rs");

pub mod after;

#[cfg(feature = "root-unqualified")]
pub fn root_unqualified() -> mf2::Tr {
    tr!("hello")
}
