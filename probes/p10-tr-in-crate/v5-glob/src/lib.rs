//! Variant 5: which `tr` wins.

#[cfg(feature = "root-glob")]
#[allow(unused_imports)]
use v5_prelude::prelude::*;

pub mod before;

mf2::include_generated!("mf2_generated_v5.rs");

pub mod after;

#[cfg(feature = "root-glob")]
pub fn root() -> mf2::Tr {
    tr!("hello")
}
