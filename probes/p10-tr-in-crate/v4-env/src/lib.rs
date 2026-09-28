//! Variant 4: the env-driven stand-ins, by path, from anywhere in the crate.

pub mod before;

mf2::include_generated!();

pub mod after;

/// The crate root, after the include.
pub fn root() -> mf2::Tr {
    v4_env_macro::tr_env!("hello")
}
