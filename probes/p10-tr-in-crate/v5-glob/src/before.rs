//! Declared before the include.

#[cfg(feature = "before-glob")]
pub fn before() -> mf2::Tr {
    use v5_prelude::prelude::*;
    tr!("hello")
}
